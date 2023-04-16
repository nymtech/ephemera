use clap::Parser;
use ephemera::configuration::Configuration;
use log::info;
use tokio::signal::unix::{signal, SignalKind};

use nym_api::contract::SmartContract;
use nym_api::ContractArgs;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let args = ContractArgs::parse();
    let ephemera_config = Configuration::try_load(args.ephemera_config.clone().into()).unwrap();

    let sh = tokio::spawn(async move { SmartContract::start(args, ephemera_config).await });

    let mut stream_int = signal(SignalKind::interrupt()).unwrap();
    let mut stream_term = signal(SignalKind::terminate()).unwrap();
    tokio::select! {
        _ = stream_int.recv() => {
        }
        _ = stream_term.recv() => {
        }
        _ = sh => {
            info!("Smart contract exited");
        }
    }

    info!("Exiting smart contract");
}
