use clap::Parser;

use crate::config::{
    BlockConfig, BroadcastConfig, Configuration, DbConfig, HttpConfig, Libp2pConfig, NodeConfig,
    WsConfig,
};
use crate::crypto::{EphemeraKeypair, EphemeraPublicKey, Keypair};

//network settings
const DEFAULT_LISTEN_ADDRESS: &str = "/ip4/127.0.0.1/tcp/";
const DEFAULT_LISTEN_PORT: &str = "3000";

//libp2p settings
const DEFAULT_PROPOSED_MSG_TOPIC_NAME: &str = "nym-ephemera-proposed";
const DEFAULT_HEARTBEAT_INTERVAL_SEC: u64 = 1;

//protocol settings
const DEFAULT_QUORUM_THRESHOLD_COUNT: usize = 1;
const DEFAULT_TOTAL_NR_OF_NODES: usize = 1;

#[derive(Debug, Clone, Parser)]
pub struct InitCmd {
    #[arg(long, default_value = "default")]
    pub node: String,
    #[clap(long, default_value = DEFAULT_LISTEN_ADDRESS)]
    pub address: String,
    #[clap(long, default_value = DEFAULT_LISTEN_PORT)]
    pub port: u16,
    #[clap(long, default_value_t = DEFAULT_QUORUM_THRESHOLD_COUNT)]
    pub quorum_threshold_count: usize,
    #[clap(long, default_value_t = DEFAULT_TOTAL_NR_OF_NODES)]
    pub total_nr_of_nodes: usize,
    #[clap(long)]
    pub rocksdb_path: String,
    #[clap(long)]
    pub sqlite_path: String,
    #[clap(long)]
    pub ws_address: String,
    #[clap(long)]
    pub network_client_listener_address: String,
    #[clap(long)]
    pub http_server_address: String,
    #[clap(long, default_value_t = true)]
    pub block_producer: bool,
    #[clap(long, default_value_t = 15)]
    pub block_creation_interval_sec: u64,
}

impl InitCmd {
    pub fn execute(self) {
        if Configuration::try_load_from_home_dir(&self.node).is_ok() {
            panic!("Configuration file already exists: {}", self.node);
        }

        let keypair = Keypair::generate(None);
        let public_key = keypair.public_key().to_base58();
        let private_key = keypair.to_base58();

        let configuration = Configuration {
            node: NodeConfig {
                address: format!("{}{}", self.address, self.port),
                public_key,
                private_key,
            },
            broadcast: BroadcastConfig {
                cluster_size: self.total_nr_of_nodes,
            },
            libp2p: Libp2pConfig {
                ephemera_msg_topic_name: DEFAULT_PROPOSED_MSG_TOPIC_NAME.to_string(),
                heartbeat_interval_sec: DEFAULT_HEARTBEAT_INTERVAL_SEC,
                peers: vec![],
            },
            storage: DbConfig {
                rocket_path: self.rocksdb_path,
                sqlite_path: self.sqlite_path,
                create_if_not_exists: true,
            },
            websocket: WsConfig {
                ws_address: self.ws_address,
            },
            http: HttpConfig {
                address: self.http_server_address,
            },
            block: BlockConfig {
                producer: self.block_producer,
                creation_interval_sec: self.block_creation_interval_sec,
            },
        };
        if let Err(err) = configuration.try_create(&self.node) {
            eprintln!("Error creating configuration file: {err:?}", );
        }
    }
}
