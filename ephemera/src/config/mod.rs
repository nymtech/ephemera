pub mod configuration;

//network settings
pub const DEFAULT_LISTEN_ADDRESS: &str = "/ip4/127.0.0.1/tcp/";
pub const DEFAULT_LISTEN_PORT: &str = "3000";

//libp2p settings
pub const DEFAULT_TOPIC_NAME: &str = "nym-ephemera";
pub const DEFAULT_HEARTBEAT_INTERVAL_SEC: u64 = 1;

//protocol settings
pub const DEFAULT_QUORUM_THRESHOLD_COUNT: usize = 1;
pub const DEFAULT_TOTAL_NR_OF_NODES: usize = 1;
