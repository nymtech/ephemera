use clap::Parser;

use ephemera::cli::Cli;
use ephemera::helpers::init_logging;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging();

    Cli::parse().execute().await?;
    Ok(())
}
