use crate::config::configuration::{BroadcastProtocolSettings, Configuration, Libp2pSettings};
use clap::Parser;

use crate::config::{
    DEFAULT_HEARTBEAT_INTERVAL_SEC, DEFAULT_LISTEN_ADDRESS, DEFAULT_LISTEN_PORT,
    DEFAULT_QUORUM_THRESHOLD_COUNT, DEFAULT_TOPIC_NAME, DEFAULT_TOTAL_NR_OF_NODES,
};
use crate::crypto::libp2p2_crypto::{KeyPair, Libp2pKeypair};

#[derive(Debug, Clone, Parser)]
pub struct InitCmd {
    #[arg(long, default_value = "default")]
    pub name: String,
    #[clap(short, long, default_value = DEFAULT_LISTEN_ADDRESS)]
    pub address: String,
    #[clap(short, long, default_value = DEFAULT_LISTEN_PORT)]
    pub port: u16,
    #[clap(short, long, default_value_t = DEFAULT_QUORUM_THRESHOLD_COUNT)]
    pub quorum_threshold_count: usize,
    #[clap(short, long, default_value_t = DEFAULT_TOTAL_NR_OF_NODES)]
    pub total_nr_of_nodes: usize,
}

impl InitCmd {
    pub fn execute(self) {
        if Configuration::try_load_from_home_dir(&self.name).is_ok() {
            log::error!("Configuration file already exists: {:?}", Configuration::ephemera_config_file(&self.name).unwrap());
            return;
        }
        let keypair = Libp2pKeypair::generate(&[]).unwrap();
        let pub_key = hex::encode(keypair.0.public().to_protobuf_encoding());
        let priv_key = hex::encode(keypair.0.to_protobuf_encoding().unwrap());

        let configuration = Configuration {
            address: format!("{}{}", self.address, self.port),
            pub_key,
            priv_key,
            quorum: BroadcastProtocolSettings {
                quorum_threshold_size: self.quorum_threshold_count,
                cluster_size: self.total_nr_of_nodes,
            },
            libp2p: Libp2pSettings {
                topic_name: DEFAULT_TOPIC_NAME.to_string(),
                heartbeat_interval_sec: DEFAULT_HEARTBEAT_INTERVAL_SEC,
                peers: Vec::new(),
            },
        };
        if let Err(err) = configuration.try_create(&self.name){
            log::error!("Error creating configuration file: {:?}", err);
        }
    }
}
