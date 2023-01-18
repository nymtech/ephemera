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
        #[clap(short, long)]
        sleep_time_sec: u64 },
}

pub fn parse_args() -> Args {
    Args::parse()
}
