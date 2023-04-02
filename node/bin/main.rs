use clap::Parser;

use ephemera::cli::{Cli, Subcommand};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

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
            return run_node.execute().await;
        }
        Subcommand::GenerateKeypair(gen_keypair) => {
            gen_keypair.execute().await;
        }
        Subcommand::UpdateConfig(update_config) => {
            update_config.execute().await;
        }
    }
    Ok(())
}
