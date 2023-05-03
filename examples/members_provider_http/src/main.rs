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
    all: bool,
    #[clap(long)]
    reduced: Option<f64>,
    #[clap(long)]
    healthy: bool,
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
        ProviderArgs { all: true, .. } => Box::new(PeersProvider),
        ProviderArgs {
            reduced: Some(reduced),
            ..
        } => Box::new(ReducingPeerProvider::new(*reduced)),
        ProviderArgs { healthy: true, .. } => Box::new(HealthCheckPeersProvider),
        _ => panic!("Invalid provider args"),
    }
}
