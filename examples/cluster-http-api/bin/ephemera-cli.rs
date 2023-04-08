use clap::Parser;
use ephemera::cli::Cli;

#[tokio::main]
async fn main() {
    Cli::parse().execute().await.unwrap();
}
