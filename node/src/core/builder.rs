use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::api::application::Application;
use crate::api::{ApiListener, EphemeraExternalApi};
use crate::block::manager::{BlockManager, BlockManagerBuilder};
use crate::broadcast::bracha::broadcaster::Broadcaster;
use crate::config::Configuration;
use crate::core::shutdown::{Shutdown, ShutdownHandle, ShutdownManager};
use crate::database::rocksdb::RocksDbStorage;
use crate::network::libp2p::messages_channel::{NetCommunicationReceiver, NetCommunicationSender};
use crate::network::libp2p::swarm::SwarmNetwork;
use crate::utilities::crypto::key_manager::KeyManager;
use crate::utilities::{Ed25519Keypair, PeerId, ToPeerId};
use crate::websocket::ws_manager::{WsManager, WsMessageBroadcaster};
use crate::{http, Ephemera};

pub(crate) struct InstanceInfo {
    pub(crate) peer_id: PeerId,
    pub(crate) keypair: Arc<Ed25519Keypair>,
}

impl InstanceInfo {
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

pub struct EphemeraStarter {
    config: Configuration,
    instance_info: InstanceInfo,
    block_manager_builder: Option<BlockManagerBuilder>,
    block_manager: Option<BlockManager>,
    broadcaster: Broadcaster,
    from_network: Option<NetCommunicationReceiver>,
    to_network: Option<NetCommunicationSender>,
    ws_message_broadcast: Option<WsMessageBroadcaster>,
    storage: Option<RocksDbStorage>,
    api_listener: ApiListener,
    api: EphemeraExternalApi,
}

impl EphemeraStarter {
    //Crate pure data structures, no resource allocation nor threads
    pub fn new(config: Configuration) -> anyhow::Result<Self> {
        let keypair = KeyManager::read_keypair(config.node_config.private_key.clone())?;
        let peer_id = keypair.peer_id();
        log::info!("Local node id: {}", peer_id);

        let instance_info = InstanceInfo::new(peer_id, keypair.clone());

        let broadcaster = Broadcaster::new(config.clone(), peer_id, keypair);

        let block_manager_builder = BlockManagerBuilder::new(config.block_config.clone(), peer_id);

        let (api, api_listener) = EphemeraExternalApi::new();

        let builder = EphemeraStarter {
            config,
            instance_info,
            block_manager_builder: Some(block_manager_builder),
            block_manager: None,
            broadcaster,
            from_network: None,
            to_network: None,
            ws_message_broadcast: None,
            storage: None,
            api_listener,
            api,
        };
        Ok(builder)
    }

    //opens database and spawns dependent tasks
    pub async fn init_tasks<A: Application>(self, application: A) -> anyhow::Result<Ephemera<A>> {
        let starter = self.connect_db().await?;
        let mut builder = starter.init_block_manager().await?;

        let (mut shutdown_manager, shutdown_handle) = ShutdownManager::init();
        builder.start_tasks(&mut shutdown_manager).await?;

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
        let http_task = self.start_http(shutdown_manager.subscribe());
        shutdown_manager.add_handle(http_task);
        ////////////////////////////////////////////////////////////////////////

        ////////////////////////// WEBSOCKET /////////////////////////////////////
        log::info!("Starting websocket listener...");
        let ws_task = self.start_websocket(shutdown_manager.subscribe());
        shutdown_manager.add_handle(ws_task);

        Ok(())
    }

    fn start_websocket(&mut self, mut shutdown: Shutdown) -> JoinHandle<()> {
        let (websocket, ws_message_broadcast) = WsManager::new(self.config.ws_config.clone());
        self.ws_message_broadcast = Some(ws_message_broadcast);

        tokio::spawn(async move {
            tokio::select! {
                _ = shutdown.shutdown_signal_rcv.recv() => {
                    log::info!("Shutting down websocket manager");
                }
                ws_stopped = websocket.bind() => {
                    match ws_stopped {
                        Ok(_) => log::info!("Websocket stopped unexpectedly"),
                        Err(e) => log::error!("Websocket stopped with error: {}", e),
                    }
                }
            }
            log::info!("Websocket task finished");
        })
    }

    fn start_http(&mut self, mut shutdown: Shutdown) -> JoinHandle<()> {
        //TODO: handle unwrap, halt startup
        let http = http::init(self.config.http_config.clone(), self.api.clone()).unwrap();

        tokio::spawn(async move {
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
        })
    }

    fn start_network(&mut self, mut shutdown: Shutdown) -> anyhow::Result<JoinHandle<()>> {
        let (mut network, from_network, to_network) =
            SwarmNetwork::new(self.config.clone(), self.instance_info.keypair.clone());
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

    fn ephemera<A: Application>(
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
            instance_info: self.instance_info,
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
        let database = RocksDbStorage::open(self.config.db_config.clone())?;
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
