use clap::Parser;

use ephemera::membership::PeerSetting;

use crate::http::run_peers_http_server;
use crate::peers::read_peers_config;
use crate::provider::{PeersProvider, ReducingPeerProvider};

mod http;
mod peers;
mod provider;

#[derive(Parser, Clone)]
#[command(name = "Members provider http")]
#[command(about = "Ephemera members provider http example", long_about = None)]
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

#[tokio::main]
async fn main() {
    let _args = Args::parse();
    let peers = read_peers_config();
    let _provider = PeersProvider::new(peers.clone());
    let provider = ReducingPeerProvider::new(peers, 0.4);

    run_peers_http_server(provider).await;
}
