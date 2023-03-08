//! Configuration options for the node.

use std::io::Write;
use std::path::PathBuf;

use serde_derive::{Deserialize, Serialize};
use thiserror::Error;

//network settings
pub const DEFAULT_LISTEN_ADDRESS: &str = "/ip4/127.0.0.1/tcp/";
pub const DEFAULT_LISTEN_PORT: &str = "3000";

//libp2p settings
pub const DEFAULT_CONSENSUS_MSG_TOPIC_NAME: &str = "nym-ephemera-consensus";
pub const DEFAULT_PROPOSED_MSG_TOPIC_NAME: &str = "nym-ephemera-proposed";
pub const DEFAULT_HEARTBEAT_INTERVAL_SEC: u64 = 1;

//protocol settings
pub const DEFAULT_QUORUM_THRESHOLD_COUNT: usize = 1;
pub const DEFAULT_TOTAL_NR_OF_NODES: usize = 1;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Configuration {
    pub node_config: NodeConfig,
    pub quorum: BroadcastProtocolSettings,
    pub libp2p: Libp2pSettings,
    pub db_config: DbConfig,
    pub ws_config: WsConfig,
    pub http_config: HttpConfig,
    pub block_config: BlockConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NodeConfig {
    pub address: String,
    pub pub_key: String,
    //TODO: dev only
    pub private_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BroadcastProtocolSettings {
    pub cluster_size: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Libp2pSettings {
    pub consensus_msg_topic_name: String,
    pub proposed_msg_topic_name: String,
    pub heartbeat_interval_sec: u64,
    pub peers: Vec<PeerSetting>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PeerSetting {
    pub name: String,
    pub address: String,
    pub pub_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DbConfig {
    pub sqlite_path: String,
    pub rocket_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WsConfig {
    /// Address to listen on for WebSocket API requests
    pub ws_address: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct HttpConfig {
    /// Address to listen on for HTTP API requests
    pub address: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BlockConfig {
    /// By default every node is block producer.
    /// But it may be useful for some applications to have only one.
    /// We may later also introduce a mechanism and algorithm to elect block producers.
    pub block_producer: bool,
    /// Interval in seconds between block creation
    /// Blocks are "proposed" at this interval.
    pub block_creation_interval_sec: u64,
}

#[derive(Debug, Error)]
pub enum ConfigurationError {
    #[error("Configuration file exists: '{}'", .0)]
    ConfigurationFileExists(String),
    #[error("Configuration file does not exists: '{}'", .0)]
    ConfigurationFileDoesNotExists(String),
    #[error("Configuration file does not exist")]
    IoError(#[from] std::io::Error),
    #[error("{}", .0)]
    Other(String),
}

const EPHEMERA_DIR_NAME: &str = ".ephemera";
const EPHEMERA_CONFIG_FILE: &str = "ephemera.toml";

type Result<T> = std::result::Result<T, ConfigurationError>;

impl Configuration {
    pub fn try_load(file: PathBuf) -> Result<Configuration> {
        let config = config::Config::builder()
            .add_source(config::File::from(file))
            .build()
            .map_err(|e| ConfigurationError::Other(e.to_string()))?;

        config
            .try_deserialize()
            .map_err(|e| ConfigurationError::Other(e.to_string()))
    }

    pub fn try_load_node(node_name: &str, file: &str) -> Result<Configuration> {
        let file_path = Self::ephemera_node_dir(node_name)?.join(file);
        Configuration::try_load(file_path)
    }

    pub fn try_load_from_home_dir(node_name: &str) -> Result<Configuration> {
        let file_path = Configuration::ephemera_config_file(node_name)?;
        let config = config::Config::builder()
            .add_source(config::File::from(file_path))
            .build()
            .map_err(|e| ConfigurationError::Other(e.to_string()))?;

        config
            .try_deserialize()
            .map_err(|e| ConfigurationError::Other(e.to_string()))
    }

    pub fn try_create(&self, node_name: &str) -> Result<()> {
        let conf_path = Configuration::ephemera_node_dir(node_name)?;
        if !conf_path.exists() {
            std::fs::create_dir_all(conf_path)?;
        }

        let file_path = Configuration::ephemera_config_file(node_name)?;
        if file_path.exists() {
            return Err(ConfigurationError::ConfigurationFileExists(
                file_path.to_str().unwrap().to_string(),
            ));
        }

        self.write(file_path)?;
        Ok(())
    }

    pub fn try_update(&self, node_name: &str) -> Result<()> {
        let file_path = Configuration::ephemera_config_file(node_name)?;
        if !file_path.exists() {
            log::error!(
                "Configuration file does not exist {}",
                file_path.to_str().unwrap()
            );
            return Err(ConfigurationError::ConfigurationFileDoesNotExists(
                file_path.to_str().unwrap().to_string(),
            ));
        }
        self.write(file_path)?;
        Ok(())
    }

    fn ephemera_dir() -> Result<PathBuf> {
        dirs::home_dir()
            .map(|home| home.join(EPHEMERA_DIR_NAME))
            .ok_or(ConfigurationError::Other(
                "Could not find home directory".to_string(),
            ))
    }

    fn ephemera_node_dir(node_name: &str) -> Result<PathBuf> {
        Ok(Self::ephemera_dir()?.join(node_name))
    }

    pub fn ephemera_config_file(node_name: &str) -> Result<PathBuf> {
        Ok(Self::ephemera_node_dir(node_name)?.join(EPHEMERA_CONFIG_FILE))
    }

    fn write(&self, file_path: PathBuf) -> Result<()> {
        let config = toml::to_string(&self).map_err(|e| {
            ConfigurationError::Other(format!("Failed to serialize configuration: {e}",))
        })?;

        let config = format!(
            "#This file is generated by cli and automatically overwritten every time when cli is run\n{config}",
        );

        if file_path.exists() {
            log::info!("Updating configuration file: '{}'", file_path.display());
        } else {
            log::info!("Writing configuration to file: '{}'", file_path.display());
        }

        let mut file = std::fs::File::create(&file_path)?;
        file.write_all(config.as_bytes())?;

        Ok(())
    }
}
