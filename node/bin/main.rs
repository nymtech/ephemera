use clap::Parser;
use ephemera::cli::{Cli, Subcommand};
use ephemera::logging::init_logging;

#[tokio::main]
async fn main() {
    init_logging();

    let cli = Cli::parse();
    match cli.subcommand {
        Subcommand::Init(init) => {
            init.execute();
        }
        Subcommand::AddPeer(add_peer) => {
            add_peer.execute();
        }
        Subcommand::AddLocalPeers(add_local_peers) => {
            add_local_peers.execute();
        }
        Subcommand::RunNode(run_node) => {
            run_node.execute().await;
        }
    }
}
