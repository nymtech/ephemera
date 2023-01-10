use std::env;
use clap::Parser;
use ephemera::cli::{Cli, Subcommand};

fn main() {
    if !env::vars().any(|(k, _)| k == "RUST_LOG") {
        env::set_var("RUST_LOG", "info");
    }

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
    }
}
