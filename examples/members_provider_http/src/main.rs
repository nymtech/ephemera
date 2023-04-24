use clap::{Args, Parser};
use tokio::sync::mpsc::channel;

use ephemera::membership::PeerSetting;

use crate::http::run_peers_http_server;
use crate::provider::{
    HealthCheckPeersProvider, PeersProvider, Provider, ProviderRunner, ReducingPeerProvider,
};

mod http;
mod provider;

#[derive(Args)]
#[group(required = true, multiple = false)]
struct ProviderArgs {
    #[clap(long)]
    all: Option<bool>,
    #[clap(long)]
    reduced: Option<bool>,
    #[clap(long)]
    healthy: Option<bool>,
}

#[derive(Parser)]
#[command(name = "Members provider http")]
#[command(about = "Ephemera members provider http example", long_about = None)]
#[command(next_line_help = true)]
struct RunProviderArgs {
    #[command(flatten)]
    provider: ProviderArgs,
}

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
async fn main() -> anyhow::Result<()> {
    let args = RunProviderArgs::parse();

    let provider = get_provider(&args.provider).await;

    let (tx, rcv) = channel(10);
    let runner = ProviderRunner::new(provider, rcv);
    let runner_handle = tokio::spawn(runner.run());

    let provider_handle = tokio::spawn(run_peers_http_server(tx.clone()));

    tokio::select! {
        res = provider_handle => {
            println!("Provider http server stopped: {res:?}");
        }
        res = runner_handle => {
            println!("Provider runner stopped: {res:?}");
        }
    }

    Ok(())
}

async fn get_provider(args: &ProviderArgs) -> Box<dyn Provider> {
    match args {
        ProviderArgs { all: Some(_), .. } => Box::new(PeersProvider::new()),
        ProviderArgs {
            reduced: Some(_), ..
        } => Box::new(ReducingPeerProvider::new(0.7)),
        ProviderArgs {
            healthy: Some(_), ..
        } => Box::new(HealthCheckPeersProvider::new()),
        _ => panic!("No provider selected"),
    }
}
