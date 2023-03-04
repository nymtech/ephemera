use clap::Parser;
use tokio::signal::unix::{signal, SignalKind};

use nym_api::contract::SmartContract;
use nym_api::ContractArgs;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let args = ContractArgs::parse();

    let sh = tokio::spawn(async move {
        SmartContract::start(args.url, args.db_path, args.epoch_duration_seconds).await
    });

    let mut stream_int = signal(SignalKind::interrupt()).unwrap();
    let mut stream_term = signal(SignalKind::terminate()).unwrap();
    tokio::select! {
        _ = stream_int.recv() => {
        }
        _ = stream_term.recv() => {
        }
        _ = sh => {
            log::info!("Smart contract exited");
        }
    }

    log::info!("Exiting smart contract");
}
