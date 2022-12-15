use std::env;

use serde_derive::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Settings {
    pub address: String,
    pub client_listener: String,
    pub pub_key: String,
    pub private_key: String,
    pub signatures_file: String,
    pub quorum: QuorumSettings,
    pub gossipsub: GossipSubSettings,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct QuorumSettings {
    pub threshold: u64,
    pub total: u64,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct GossipSubSettings {
    pub topic_name: String,
    pub peers: Vec<PeerSetting>,
    pub heartbeat_interval_sec: u64,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct PeerSetting {
    pub name: String,
    pub address: String,
    pub pub_key: String,
}

impl Settings {
    pub fn load(config_dir: &str, file: &str) -> Settings {
        let configuration_directory = env::current_dir()
            .expect("Failed to determine the current directory")
            .join(config_dir);
        let config = config::Config::builder()
            .add_source(config::File::from(configuration_directory.join(file)))
            .build()
            .expect("Failed to load configuration");

        config
            .try_deserialize()
            .expect("Failed to deserialize configuration")
    }
}
