use std::fmt::Display;
use std::future::Future;
use std::sync::Arc;

use futures_util::future::BoxFuture;
use futures_util::FutureExt;
use log::{error, info};
use tokio::sync::Mutex;

use crate::{
    api::{ApiListener, application::Application, CommandExecutor, http},
    block::{builder::BlockManagerBuilder, manager::BlockManager},
    broadcast::bracha::broadcast::Broadcaster,
    broadcast::group::BroadcastGroup,
    config::Configuration,
    core::{
        api_cmd::ApiCmdProcessor,
        shutdown::{Handle, Shutdown, ShutdownManager},
    },
    crypto::Keypair,
    Ephemera,
    membership,
    membership::PeerInfo,
    network::libp2p::{
        ephemera_sender::EphemeraToNetworkSender, network_sender::NetCommunicationReceiver,
        swarm_network::SwarmNetwork,
    },
    peer::{PeerId, ToPeerId},
    storage::EphemeraDatabase,
    utilities::crypto::key_manager::KeyManager,
    websocket::ws_manager::{WsManager, WsMessageBroadcaster},
};
#[cfg(feature = "rocksdb_storage")]
use crate::storage::rocksdb::RocksDbStorage;
#[cfg(feature = "sqlite_storage")]
use crate::storage::sqlite::SqliteStorage;

#[derive(Clone)]
pub(crate) struct NodeInfo {
    pub(crate) ip: String,
    pub(crate) protocol_port: u16,
    pub(crate) http_port: u16,
    pub(crate) ws_port: u16,
    pub(crate) peer_id: PeerId,
    pub(crate) keypair: Arc<Keypair>,
    pub(crate) initial_config: Configuration,
}

impl NodeInfo {
    pub(crate) fn new(config: Configuration) -> anyhow::Result<Self> {
        let keypair = KeyManager::read_keypair_from_str(&config.node.private_key)?;
        let info = Self {
            ip: config.node.ip.clone(),
            protocol_port: config.libp2p.port,
            http_port: config.http.port,
            ws_port: config.websocket.port,
            peer_id: keypair.peer_id(),
            keypair,
            initial_config: config,
        };
        Ok(info)
    }

    pub(crate) fn protocol_address(&self) -> String {
        format!("/ip4/{}/tcp/{}", self.ip, self.protocol_port)
    }

    pub(crate) fn api_address_http(&self) -> String {
        format!("http://{}:{}", self.ip, self.http_port)
    }

    pub(crate) fn ws_address_ws(&self) -> String {
        format!("ws://{}:{}", self.ip, self.ws_port)
    }

    pub(crate) fn ws_address_ip_port(&self) -> String {
        format!("{}:{}", self.ip, self.ws_port)
    }
}

impl Display for NodeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "NodeInfo {{ ip: {}, protocol_port: {}, http_port: {}, ws_port: {}, peer_id: {}, keypair: {} }}",
            self.ip,
            self.protocol_port,
            self.http_port,
            self.ws_port,
            self.peer_id,
            self.keypair
        )
    }
}

#[derive(Clone)]
pub struct EphemeraHandle {
    /// Ephemera API
    pub api: CommandExecutor,
    /// Allows to send shutdown signal to the node
    pub shutdown: Handle,
}

pub struct EphemeraStarter<A, P>
    where
        A: Application + 'static,
        P: Future<Output=membership::Result<Vec<PeerInfo>>> + Send + 'static,
{
    config: Configuration,
    node_info: NodeInfo,
    block_manager_builder: Option<BlockManagerBuilder>,
    block_manager: Option<BlockManager>,
    broadcaster: Broadcaster,
    members_provider: Option<P>,
    application: Option<A>,
    from_network: Option<NetCommunicationReceiver>,
    to_network: Option<EphemeraToNetworkSender>,
    ws_message_broadcast: Option<WsMessageBroadcaster>,
    storage: Option<Box<dyn EphemeraDatabase>>,
    api_listener: ApiListener,
    api: CommandExecutor,
    services: Vec<BoxFuture<'static, anyhow::Result<()>>>,
}

impl<A, P> EphemeraStarter<A, P>
    where
        A: Application + 'static,
        P: Future<Output=membership::Result<Vec<PeerInfo>>> + Send + Unpin + 'static,
{
    /// Creates a new Ephemera node builder.
    ///
    /// # Arguments
    /// * `config` - Configuration of the node
    ///
    /// # Returns
    /// * `EphemeraStarter` - Builder of the node
    ///
    /// # Errors
    /// * `anyhow::Error` - If the node info cannot be created
    pub fn new(config: Configuration) -> anyhow::Result<Self> {
        let instance_info = NodeInfo::new(config.clone())?;

        let broadcaster = Broadcaster::new(instance_info.peer_id);

        let block_manager_builder =
            BlockManagerBuilder::new(config.block.clone(), instance_info.keypair.clone());

        let (api, api_listener) = CommandExecutor::new();

        let builder = EphemeraStarter {
            config,
            node_info: instance_info,
            block_manager_builder: Some(block_manager_builder),
            block_manager: None,
            broadcaster,
            members_provider: None,
            application: None,
            from_network: None,
            to_network: None,
            ws_message_broadcast: None,
            storage: None,
            api_listener,
            api,
            services: vec![],
        };
        Ok(builder)
    }

    #[must_use]
    pub fn with_members_provider(self, members_provider: P) -> Self
        where
            P: Future<Output=membership::Result<Vec<PeerInfo>>> + Send + 'static,
    {
        Self {
            members_provider: Some(members_provider),
            ..self
        }
    }

    #[must_use]
    pub fn with_application(self, application: A) -> Self
        where
            A: Application + 'static,
    {
        Self {
            application: Some(application),
            ..self
        }
    }

    /// Starts all Ephemera internal services except the Ephemera itself.
    ///
    /// # Returns
    /// * `Ephemera` - Ephemera instance
    ///
    /// # Errors
    /// * `anyhow::Error` - If some of the services cannot be started
    ///
    /// # Panics
    /// * If builder is not initialized properly
    pub fn build(self) -> anyhow::Result<Ephemera<A>> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "sqlite_storage")] {
                let starter = self.connect_sqlite()?;
            } else if #[cfg(feature = "rocksdb_storage")] {
                let starter = self.connect_rocksdb().await?;
            } else {
                compile_error!("Must enable either sqlite or rocksdb feature");
            }
        }

        let mut builder = starter.init_block_manager()?;

        let (mut shutdown_manager, shutdown_handle) = ShutdownManager::init();

        builder.init_services(&mut shutdown_manager)?;

        let ephemera = builder.ephemera(shutdown_handle, shutdown_manager);
        Ok(ephemera)
    }

    fn init_services(&mut self, shutdown_manager: &mut ShutdownManager) -> anyhow::Result<()> {
        self.services = vec![
            self.init_libp2p(shutdown_manager.subscribe()),
            self.init_http(shutdown_manager.subscribe())?,
            self.init_websocket(shutdown_manager.subscribe()),
        ];
        Ok(())
    }

    fn init_websocket(&mut self, mut shutdown: Shutdown) -> BoxFuture<'static, anyhow::Result<()>> {
        let (mut websocket, ws_message_broadcast) =
            WsManager::new(self.node_info.ws_address_ip_port());

        self.ws_message_broadcast = Some(ws_message_broadcast);

        async move {
            websocket.listen().await?;

            tokio::select! {
                _ = shutdown.shutdown_signal_rcv.recv() => {
                    info!("Shutting down websocket manager");
                }
                ws_stopped = websocket.run() => {
                    match ws_stopped {
                        Ok(_) => info!("Websocket stopped unexpectedly"),
                        Err(e) => error!("Websocket stopped with error: {}", e),
                    }
                }
            }
            info!("Websocket task finished");
            Ok(())
        }
            .boxed()
    }

    fn init_http(
        &mut self,
        mut shutdown: Shutdown,
    ) -> anyhow::Result<BoxFuture<'static, anyhow::Result<()>>> {
        let http = http::init(&self.node_info, self.api.clone())?;

        let fut = async move {
            let server_handle = http.handle();
            tokio::select! {
                _ = shutdown.shutdown_signal_rcv.recv() => {
                    info!("Shutting down http server");
                    server_handle.stop(true).await;
                }
                http_stopped = http => {
                    match http_stopped {
                        Ok(_) => info!("Http server stopped unexpectedly"),
                        Err(e) => error!("Http server stopped with error: {}", e),
                    }
                }
            }
            info!("Http task finished");
            Ok(())
        }
            .boxed();
        Ok(fut)
    }

    fn init_libp2p(&mut self, mut shutdown: Shutdown) -> BoxFuture<'static, anyhow::Result<()>> {
        info!("Starting network...",);

        let (mut network, from_network, to_network) = SwarmNetwork::new(
            self.node_info.clone(),
            self.members_provider.take().unwrap(),
        );

        self.from_network = Some(from_network);
        self.to_network = Some(to_network);

        async move {
            network.listen()?;

            tokio::select! {
                _ = shutdown.shutdown_signal_rcv.recv() => {
                    info!("Shutting down network");
                }
                nw_stopped = network.start() => {
                    match nw_stopped {
                        Ok(_) => info!("Network stopped unexpectedly"),
                        Err(e) => error!("Network stopped with error: {e}",),
                    }
                }
            }
            info!("Network task finished");
            Ok(())
        }
            .boxed()
    }

    fn ephemera(
        mut self,
        shutdown_handle: Handle,
        shutdown_manager: ShutdownManager,
    ) -> Ephemera<A> {
        let ephemera_handle = EphemeraHandle {
            api: self.api,
            shutdown: shutdown_handle,
        };
        //TODO: builder pattern should make (statically) sure that all unwraps are satisfied
        //TODO make unwrap safe by Builder type system
        let application = self.application.take().unwrap();
        Ephemera {
            node_info: self.node_info,
            block_manager: self.block_manager.unwrap(),
            broadcaster: self.broadcaster,
            from_network: self.from_network.unwrap(),
            to_network: self.to_network.unwrap(),
            broadcast_group: BroadcastGroup::new(),
            storage: Arc::new(Mutex::new(self.storage.unwrap())),
            ws_message_broadcast: self.ws_message_broadcast.unwrap(),
            api_listener: self.api_listener,
            api_cmd_processor: ApiCmdProcessor::new(),
            application: application.into(),
            ephemera_handle,
            shutdown_manager,
            services: self.services,
        }
    }

    //allocate database connection
    #[cfg(feature = "rocksdb_storage")]
    fn connect_rocksdb(mut self) -> anyhow::Result<Self> {
        info!("Opening database...");
        let database = RocksDbStorage::open(self.config.storage.clone())?;
        self.storage = Some(Box::new(database));
        Ok(self)
    }

    #[cfg(feature = "sqlite_storage")]
    fn connect_sqlite(mut self) -> anyhow::Result<Self> {
        info!("Opening database...");
        let database = SqliteStorage::open(self.config.storage.clone())?;
        self.storage = Some(Box::new(database));
        Ok(self)
    }

    fn init_block_manager(mut self) -> anyhow::Result<Self> {
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
                let block_manager = bm_builder.build(&mut *db)?;
                self.block_manager = Some(block_manager);
            }
        }
        self.storage = Some(db);
        Ok(self)
    }
}
