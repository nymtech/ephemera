pub mod init;
pub mod peers;
pub mod run_node;

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command()]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: Subcommand,
}

#[derive(Clone, Debug, clap::Subcommand)]
pub enum Subcommand {
    Init(init::InitCmd),
    AddPeer(peers::AddPeerCmd),
    AddLocalPeers(peers::AddLocalPeersCmd),
    RunNode(run_node::RunNodeCmd),
}
