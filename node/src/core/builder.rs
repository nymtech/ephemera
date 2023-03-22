use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::api::application::Application;
use crate::api::{ApiListener, EphemeraExternalApi};
use crate::block::manager::{BlockManager, BlockManagerBuilder};
use crate::broadcast::bracha::broadcaster::Broadcaster;
use crate::config::Configuration;
use crate::core::shutdown::{Shutdown, ShutdownHandle, ShutdownManager};
use crate::network::libp2p::messages_channel::{NetCommunicationReceiver, NetCommunicationSender};
use crate::network::libp2p::swarm::SwarmNetwork;
use crate::network::{PeerDiscovery, PeerId, ToPeerId};
use crate::storage::rocksdb::RocksDbStorage;
use crate::utilities::crypto::key_manager::KeyManager;
use crate::utilities::Ed25519Keypair;
use crate::websocket::ws_manager::{WsManager, WsMessageBroadcaster};
use crate::{http, Ephemera};

#[derive(Clone)]
pub(crate) struct NodeInfo {
    pub(crate) peer_id: PeerId,
    pub(crate) keypair: Arc<Ed25519Keypair>,
}

impl NodeInfo {
    pub(crate) fn new(peer_id: PeerId, keypair: Arc<Ed25519Keypair>) -> Self {
        Self { peer_id, keypair }
    }
}

#[derive(Clone)]
pub struct EphemeraHandle {
    /// Ephemera API
    pub api: EphemeraExternalApi,
    /// Ephemera shutdown handle
    pub shutdown: ShutdownHandle,
}

pub struct EphemeraStarter<P: PeerDiscovery, A: Application> {
    config: Configuration,
    instance_info: NodeInfo,
    block_manager_builder: Option<BlockManagerBuilder>,
    block_manager: Option<BlockManager>,
    broadcaster: Broadcaster,
    peer_discovery: Option<P>,
    application: Option<A>,
    from_network: Option<NetCommunicationReceiver>,
    to_network: Option<NetCommunicationSender>,
    ws_message_broadcast: Option<WsMessageBroadcaster>,
    storage: Option<RocksDbStorage>,
    api_listener: ApiListener,
    api: EphemeraExternalApi,
}

impl<P: PeerDiscovery + 'static, A: Application + 'static> EphemeraStarter<P, A> {
    //Crate pure data structures, no resource allocation nor threads
    pub fn new(config: Configuration) -> anyhow::Result<Self> {
        let keypair = KeyManager::read_keypair(config.node.private_key.clone())?;
        let peer_id = keypair.peer_id();
        log::info!("Local node id: {}", peer_id);

        let instance_info = NodeInfo::new(peer_id, keypair.clone());

        let broadcaster = Broadcaster::new(config.broadcast.clone(), peer_id, keypair);

        let block_manager_builder = BlockManagerBuilder::new(config.block.clone(), peer_id);

        let (api, api_listener) = EphemeraExternalApi::new();

        let builder = EphemeraStarter {
            config,
            instance_info,
            block_manager_builder: Some(block_manager_builder),
            block_manager: None,
            broadcaster,
            peer_discovery: None,
            application: None,
            from_network: None,
            to_network: None,
            ws_message_broadcast: None,
            storage: None,
            api_listener,
            api,
        };
        Ok(builder)
    }

    pub fn with_peer_discovery(self, peer_discovery: P) -> Self {
        Self {
            peer_discovery: Some(peer_discovery),
            ..self
        }
    }

    pub fn with_application(self, application: A) -> EphemeraStarter<P, A> {
        Self {
            application: Some(application),
            ..self
        }
    }

    //opens database and spawns dependent tasks
    pub async fn init_tasks(self) -> anyhow::Result<Ephemera<A>> {
        let starter = self.connect_db().await?;
        let mut builder = starter.init_block_manager().await?;

        let (mut shutdown_manager, shutdown_handle) = ShutdownManager::init();
        builder.start_tasks(&mut shutdown_manager).await?;

        let application = builder.application.take().unwrap();
        let ephemera = builder.ephemera(application, shutdown_handle, shutdown_manager);
        Ok(ephemera)
    }

    //Haven't put much thought into in which order start and stop tasks.
    //(also relative to Ephemera).
    //TODO: Need to write down how all these components work together and depend on each other
    async fn start_tasks(&mut self, shutdown_manager: &mut ShutdownManager) -> anyhow::Result<()> {
        ////////////////////////// NETWORK /////////////////////////////////////
        log::info!("Starting network...");
        match self.start_network(shutdown_manager.subscribe()) {
            Ok(nw_task) => {
                shutdown_manager.add_handle(nw_task);
            }
            Err(err) => {
                log::error!("Failed to start network: {}", err);
                return Err(err);
            }
        }
        ////////////////////////////////////////////////////////////////////////

        ////////////////////////// HTTP SERVER /////////////////////////////////////
        log::info!("Starting http server...");
        match self.start_http(shutdown_manager.subscribe()) {
            Ok(http_task) => {
                shutdown_manager.add_handle(http_task);
            }
            Err(err) => {
                log::error!("Failed to start http server: {}", err);
                return Err(err);
            }
        }
        ////////////////////////////////////////////////////////////////////////

        ////////////////////////// WEBSOCKET /////////////////////////////////////
        log::info!("Starting websocket listener...");
        match self.start_websocket(shutdown_manager.subscribe()).await {
            Ok(ws_task) => {
                shutdown_manager.add_handle(ws_task);
            }
            Err(err) => {
                log::error!("Failed to start websocket: {}", err);
                return Err(err);
            }
        }
        Ok(())
    }

    async fn start_websocket(&mut self, mut shutdown: Shutdown) -> anyhow::Result<JoinHandle<()>> {
        let (mut websocket, ws_message_broadcast) = WsManager::new(self.config.websocket.clone());
        self.ws_message_broadcast = Some(ws_message_broadcast);

        websocket.listen().await?;

        let join_handle = tokio::spawn(async move {
            tokio::select! {
                _ = shutdown.shutdown_signal_rcv.recv() => {
                    log::info!("Shutting down websocket manager");
                }
                ws_stopped = websocket.run() => {
                    match ws_stopped {
                        Ok(_) => log::info!("Websocket stopped unexpectedly"),
                        Err(e) => log::error!("Websocket stopped with error: {}", e),
                    }
                }
            }
            log::info!("Websocket task finished");
        });

        Ok(join_handle)
    }

    fn start_http(&mut self, mut shutdown: Shutdown) -> anyhow::Result<JoinHandle<()>> {
        let http = http::init(self.config.http.clone(), self.api.clone())?;

        let join_handle = tokio::spawn(async move {
            let server_handle = http.handle();

            tokio::select! {
                _ = shutdown.shutdown_signal_rcv.recv() => {
                    log::info!("Shutting down http server");
                    server_handle.stop(true).await;
                }
                http_stopped = http => {
                    match http_stopped {
                        Ok(_) => log::info!("Http server stopped unexpectedly"),
                        Err(e) => log::error!("Http server stopped with error: {}", e),
                        //http_shutdown.notify_error()
                    }
                }
            }
            log::info!("Http task finished");
        });
        Ok(join_handle)
    }

    fn start_network(&mut self, mut shutdown: Shutdown) -> anyhow::Result<JoinHandle<()>> {
        log::info!("Starting network...{:?}", self.peer_discovery.is_some());
        let (mut network, from_network, to_network) = SwarmNetwork::new(
            self.config.libp2p.clone(),
            self.config.node.clone(),
            self.instance_info.clone(),
            self.peer_discovery.take().unwrap(),
        );

        self.from_network = Some(from_network);
        self.to_network = Some(to_network);

        network.listen()?;
        let join_handle = tokio::spawn(async move {
            tokio::select! {
                _ = shutdown.shutdown_signal_rcv.recv() => {
                    log::info!("Shutting down network");
                }
                nw_stopped = network.start() => {
                    match nw_stopped {
                        Ok(_) => log::info!("Network stopped unexpectedly"),
                        Err(e) => log::error!("Network stopped with error: {e}",),
                    }
                }
            }
            log::info!("Network task finished");
        });
        Ok(join_handle)
    }

    fn ephemera(
        self,
        application: A,
        shutdown_handle: ShutdownHandle,
        shutdown_manager: ShutdownManager,
    ) -> Ephemera<A> {
        let ephemera_handle = EphemeraHandle {
            api: self.api,
            shutdown: shutdown_handle,
        };
        //TODO: builder pattern needs make sure that all unwraps are safe here
        Ephemera {
            node_info: self.instance_info,
            block_manager: self.block_manager.unwrap(),
            broadcaster: self.broadcaster,
            from_network: self.from_network.unwrap(),
            to_network: self.to_network.unwrap(),
            storage: Arc::new(Mutex::new(self.storage.unwrap())),
            ws_message_broadcast: self.ws_message_broadcast.unwrap(),
            api_listener: self.api_listener,
            application: application.into(),
            ephemera_handle,
            shutdown_manager: Some(shutdown_manager),
        }
    }

    //allocate database connection
    async fn connect_db(mut self) -> anyhow::Result<Self> {
        log::info!("Opening database...");
        let database = RocksDbStorage::open(self.config.storage.clone())?;
        self.storage = Some(database);
        Ok(self)
    }

    async fn init_block_manager(mut self) -> anyhow::Result<Self> {
        let db = self.storage.take();
        if db.is_none() {
            anyhow::bail!("Database connection is not initialized")
        }
        let mut db = db.unwrap();
        let bm_builder = self.block_manager_builder.take();
        match bm_builder {
            None => {
                anyhow::bail!("Block manager builder is not initialized")
            }
            Some(bm_builder) => {
                let block_manager = bm_builder.build(&mut db)?;
                self.block_manager = Some(block_manager)
            }
        }
        self.storage = Some(db);
        Ok(self)
    }
}
