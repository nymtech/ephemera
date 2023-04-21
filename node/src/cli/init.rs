use clap::Parser;

use crate::config::{
    BlockConfig, Configuration, DbConfig, HttpConfig, Libp2pConfig, NodeConfig, WsConfig,
};
use crate::crypto::{EphemeraKeypair, Keypair};

//network settings
const DEFAULT_LISTEN_ADDRESS: &str = "127.0.0.1";
const DEFAULT_LISTEN_PORT: &str = "3000";

//libp2p settings
const DEFAULT_MESSAGES_TOPIC_NAME: &str = "nym-ephemera-proposed";
const DEFAULT_HEARTBEAT_INTERVAL_SEC: u64 = 1;

#[derive(Debug, Clone, Parser)]
pub struct InitCmd {
    #[arg(long, default_value = "default")]
    pub node_name: String,
    #[clap(long, default_value = DEFAULT_LISTEN_ADDRESS)]
    pub ip: String,
    #[clap(long, default_value = DEFAULT_LISTEN_PORT)]
    pub protocol_port: u16,
    #[clap(long)]
    pub websocket_port: u16,
    #[clap(long)]
    pub http_api_port: u16,
    #[clap(long, default_value_t = true)]
    pub block_producer: bool,
    #[clap(long, default_value_t = 30)]
    pub block_creation_interval_sec: u64,
    #[clap(long)]
    pub repeat_last_block: bool,
    #[clap(long, default_value_t = 60 * 60)]
    pub members_provider_delay_sec: u64,
}

impl InitCmd {
    pub fn execute(self) {
        if Configuration::try_load_from_home_dir(&self.node_name).is_ok() {
            panic!("Configuration file already exists: {}", self.node_name);
        }

        let path = Configuration::ephemera_root_dir()
            .unwrap()
            .join(&self.node_name);
        println!("Creating ephemera node configuration in: {:?}", path);
        println!("Configuration: {:?}", self);

        let db_dir = path.join("db");
        let rocksdb_path = db_dir.join("rocksdb");
        let sqlite_path = db_dir.join("ephemera.sqlite");
        std::fs::create_dir_all(&rocksdb_path).unwrap();
        std::fs::File::create(&sqlite_path).unwrap();

        let keypair = Keypair::generate(None);
        let private_key = keypair.to_base58();

        let configuration = Configuration {
            node: NodeConfig {
                ip: self.ip,
                private_key,
            },
            libp2p: Libp2pConfig {
                port: self.protocol_port,
                ephemera_msg_topic_name: DEFAULT_MESSAGES_TOPIC_NAME.to_string(),
                heartbeat_interval_sec: DEFAULT_HEARTBEAT_INTERVAL_SEC,
                members_provider_delay_sec: self.members_provider_delay_sec,
            },
            storage: DbConfig {
                rocket_path: rocksdb_path.as_os_str().to_str().unwrap().to_string(),
                sqlite_path: sqlite_path.as_os_str().to_str().unwrap().to_string(),
                create_if_not_exists: true,
            },
            websocket: WsConfig {
                port: self.websocket_port,
            },
            http: HttpConfig {
                port: self.http_api_port,
            },
            block: BlockConfig {
                producer: self.block_producer,
                creation_interval_sec: self.block_creation_interval_sec,
                repeat_last_block: self.repeat_last_block,
            },
        };
        if let Err(err) = configuration.try_create_root_dir(&self.node_name) {
            eprintln!("Error creating configuration file: {err:?}",);
        }
    }
}
