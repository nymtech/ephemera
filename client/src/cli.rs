use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command()]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Broadcast {
        #[clap(short, long)]
        node_address: String,
    },
}

pub fn parse_args() -> Args {
    Args::parse()
}
