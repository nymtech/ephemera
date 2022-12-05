use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command()]
pub struct Args {
    #[arg(short, long)]
    pub config_file: String,
}

pub fn parse_args() -> Args {
    Args::parse()
}
