use clap::Parser;

use crate::config::configuration::{
    BlockConfig, BroadcastProtocolSettings, Configuration, DbConfig, HttpConfig, Libp2pSettings,
    NetworkClientListenerConfig, NodeConfig, WsConfig,
};
use crate::config::{
    DEFAULT_CONSENSUS_MSG_TOPIC_NAME, DEFAULT_HEARTBEAT_INTERVAL_SEC, DEFAULT_LISTEN_ADDRESS,
    DEFAULT_LISTEN_PORT, DEFAULT_PROPOSED_MSG_TOPIC_NAME, DEFAULT_QUORUM_THRESHOLD_COUNT,
    DEFAULT_TOTAL_NR_OF_NODES,
};
use crate::utilities::crypto::libp2p2_crypto::Libp2pKeypair;
use crate::utilities::crypto::KeyPair;

#[derive(Debug, Clone, Parser)]
pub struct InitCmd {
    #[arg(long, default_value = "default")]
    pub node: String,
    #[clap(short, long, default_value = DEFAULT_LISTEN_ADDRESS)]
    pub address: String,
    #[clap(short, long, default_value = DEFAULT_LISTEN_PORT)]
    pub port: u16,
    #[clap(short, long, default_value_t = DEFAULT_QUORUM_THRESHOLD_COUNT)]
    pub quorum_threshold_count: usize,
    #[clap(short, long, default_value_t = DEFAULT_TOTAL_NR_OF_NODES)]
    pub total_nr_of_nodes: usize,
    #[clap(short, long)]
    pub sqlite_path: String,
    #[clap(short, long)]
    pub rocket_path: String,
    #[clap(short, long)]
    pub ws_address: String,
    #[clap(short, long)]
    pub network_client_listener_address: String,
    #[clap(short, long)]
    pub http_server_address: String,
    #[clap(short, long)]
    pub block_producer: bool,
}

impl InitCmd {
    pub fn execute(self) {
        if Configuration::try_load_from_home_dir(&self.node).is_ok() {
            panic!("Configuration file already exists: {}", self.node);
        }
        let keypair = Libp2pKeypair::generate().unwrap();
        let pub_key = hex::encode(keypair.0.public().to_protobuf_encoding());
        let priv_key = hex::encode(keypair.0.to_protobuf_encoding().unwrap());

        let configuration = Configuration {
            node_config: NodeConfig {
                address: format!("{}{}", self.address, self.port),
                pub_key,
                priv_key,
            },
            quorum: BroadcastProtocolSettings {
                quorum_threshold_size: self.quorum_threshold_count,
                cluster_size: self.total_nr_of_nodes,
            },
            libp2p: Libp2pSettings {
                consensus_msg_topic_name: DEFAULT_CONSENSUS_MSG_TOPIC_NAME.to_string(),
                proposed_msg_topic_name: DEFAULT_PROPOSED_MSG_TOPIC_NAME.to_string(),
                heartbeat_interval_sec: DEFAULT_HEARTBEAT_INTERVAL_SEC,
                peers: Vec::new(),
            },
            db_config: DbConfig {
                sqlite_path: self.sqlite_path,
                rocket_path: self.rocket_path,
            },
            ws_config: WsConfig {
                ws_address: self.ws_address,
            },
            network_client_listener_config: NetworkClientListenerConfig {
                address: self.network_client_listener_address,
            },
            http_config: HttpConfig {
                address: self.http_server_address,
            },
            block_config: BlockConfig {
                leader: self.block_producer,
            },
        };
        if let Err(err) = configuration.try_create(&self.node) {
            eprintln!("Error creating configuration file: {err:?}",);
        }
    }
}
