use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command()]
pub struct Args {
    #[arg(long)]
    pub broadcast: bool,
    #[arg(long)]
    pub keypair: bool,
}

pub fn parse_args() -> Args {
    Args::parse()
}
