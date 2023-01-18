use ephemera_client::{cli, run_reliable_broadcast};
use ephemera_client::cli::Commands;

#[tokio::main]
async fn main() {
    let args = cli::parse_args();
    match args.command {
        Commands::Broadcast { node_address, sleep_time_sec } => {
            let payload_generator = || {
                let mut payload = Vec::new();
                payload.extend_from_slice(b"Hello world!");
                payload
            };
            run_reliable_broadcast(node_address, sleep_time_sec, payload_generator).await;
        }
    }
}