use clap::Parser;
use std::sync::Arc;

use ephemera::configuration::Configuration;
use ephemera::membership::{JsonPeerInfo, PeerSetting};

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

#[derive(Clone)]
struct PeersProvider {
    peers: Vec<JsonPeerInfo>,
}

impl PeersProvider {
    fn new(peers: Vec<JsonPeerInfo>) -> Self {
        Self { peers }
    }

    fn peers(&self) -> Vec<JsonPeerInfo> {
        self.peers.clone()
    }
}

#[derive(Clone)]
struct ReducingPeerProvider {
    peers: Vec<JsonPeerInfo>,
    //For example if peers length is 6 and ratio is 0.2, then 1 peer will be removed.
    reduce_ratio: f64,
}

impl ReducingPeerProvider {
    fn new(peers: Vec<JsonPeerInfo>, reduce_ratio: f64) -> Self {
        Self {
            peers,
            reduce_ratio,
        }
    }

    fn peers(&self) -> Vec<JsonPeerInfo> {
        let mut reduced_peers = vec![];
        let reduce_count = (self.peers.len() as f64 * self.reduce_ratio) as usize;
        let peers_count = self.peers.len() - reduce_count;
        println!(
            "Reducing peers count from {} to {}",
            self.peers.len(),
            peers_count
        );
        for i in 0..peers_count {
            reduced_peers.push(self.peers[i].clone());
        }
        println!("Reduced peers: {:?}", reduced_peers);
        reduced_peers
    }
}

#[tokio::main]
async fn main() {
    let _args = Args::parse();
    let peers = read_peers_config();
    let _provider = PeersProvider::new(peers.clone());
    let provider = ReducingPeerProvider::new(peers, 0.4);

    run_peers_http_server(provider).await;
}

//Read config from ~/.ephemera/peers.toml
fn read_peers_config() -> Vec<JsonPeerInfo> {
    let mut peers = vec![];
    let path = Configuration::ephemera_root_dir()
        .unwrap()
        .join("peers.toml");

    let config = std::fs::read_to_string(path).unwrap();

    let mut settings = toml::from_str::<PeerSettings>(&config).unwrap();
    settings.peers.sort_by(|a, b| a.name.cmp(&b.name));

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

async fn run_peers_http_server(provider: ReducingPeerProvider) {
    let mut app = tide::with_state(Arc::new(provider));

    app.at("/peers")
        .get(|req: tide::Request<Arc<ReducingPeerProvider>>| async move {
            let provider = req.state();
            let str = serde_json::to_string(&provider.peers()).unwrap();
            Ok(str)
        });

    app.listen(format!("{}:{}", EPHEMERA_IP, PEERS_API_PORT))
        .await
        .unwrap();
}
