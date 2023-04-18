use std::sync::Arc;

use clap::Parser;

use ephemera::configuration::Configuration;
use ephemera::peer_discovery::{JsonPeerInfo, PeerSetting};

#[derive(Parser, Clone)]
#[command(name = "peer_discovery_http")]
#[command(about = "Ephemera peer discovery http resource example", long_about = None)]
#[command(next_line_help = true)]
struct Args {}

const EPHEMERA_IP: &str = "127.0.0.1";

// Node 1 port is 3000, Node2 port is 3001, etc.
const EPHEMERA_PORT_BASE: u16 = 3000;

// Node 1 http port is 7000, Node2 http port is 7001, etc.
const HTTP_API_PORT_BASE: u16 = 7000;

const PEERS_API_PORT: u16 = 8000;

#[derive(serde::Deserialize)]
struct PeerSettings {
    peers: Vec<PeerSetting>,
}

#[derive(Clone)]
struct ProvidePeers {
    peers: Vec<JsonPeerInfo>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let peers = read_peers_config();
    let provider = ProvidePeers { peers };

    run_peers_http_server(provider).await;
}

//Read config from ~/.ephemera/peers.toml
fn read_peers_config() -> Vec<JsonPeerInfo> {
    let mut peers = vec![];
    let path = Configuration::ephemera_root_dir()
        .unwrap()
        .join("peers.toml");

    let config = std::fs::read_to_string(path).unwrap();

    let settings = toml::from_str::<PeerSettings>(&config).unwrap();

    settings.peers.into_iter().for_each(|setting| {
        peers.push(JsonPeerInfo {
            name: setting.name,
            address: setting.address,
            public_key: setting.public_key,
        });
    });
    println!("Read {:?} peers from config", peers.len());
    peers
}

async fn run_peers_http_server(provider: ProvidePeers) {
    let mut app = tide::with_state(provider);

    app.at("/peers")
        .get(|req: tide::Request<ProvidePeers>| async move {
            let provider = req.state();
            let str = serde_json::to_string(&provider.peers).unwrap();
            Ok(str)
        });

    app.listen(format!("{}:{}", EPHEMERA_IP, PEERS_API_PORT))
        .await
        .unwrap();
}
