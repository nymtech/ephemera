use clap::Parser;

pub mod config;
mod crypto;
pub mod init;
pub mod peers;
pub mod run_node;

#[derive(Parser, Debug, Clone)]
#[command()]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: Subcommand,
}

#[derive(Clone, Debug, clap::Subcommand)]
pub enum Subcommand {
    InitConfig(init::InitCmd),
    InitLocalPeersConfig(peers::CreateLocalPeersConfiguration),
    RunNode(run_node::RunExternalNodeCmd),
    GenerateKeypair(crypto::GenerateKeypairCmd),
    UpdateConfig(config::UpdateConfigCmd),
}

impl Cli {
    pub async fn execute(self) -> anyhow::Result<()> {
        match self.subcommand {
            Subcommand::InitConfig(init) => {
                init.execute();
            }
            Subcommand::InitLocalPeersConfig(add_local_peers) => {
                add_local_peers.execute();
            }
            Subcommand::RunNode(run_node) => run_node.execute().await?,
            Subcommand::GenerateKeypair(gen_keypair) => {
                gen_keypair.execute();
            }
            Subcommand::UpdateConfig(update_config) => {
                update_config.execute();
            }
        }
        Ok(())
    }
}
