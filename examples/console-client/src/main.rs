use ephemera_client::cli::Commands;
use ephemera_client::{cli, RbClient};

#[tokio::main]
async fn main() {
    println!("Starting client, enter data and hit enter to broadcast...");

    let args = cli::parse_args();
    match args.command {
        Commands::Broadcast { node_address } => {
            let mut client = RbClient::new(node_address, tokio::io::stdin());
            client.run_reliable_broadcast().await;
        }
    }
}
