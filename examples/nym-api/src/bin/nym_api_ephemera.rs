//! # Nym-Api rewarding simulation with Ephemera integration
//!
//! # Overview
//!
//! It's easier to test out Ephemera integration without full Nym-Api. Simulation the actual rewarding
//! appears to be straightforward.
//!
//! # Metrics generation
//! It generates with configurable interval artificial metrics about mix-nodes availability, represented
//! as a percentage of uptime(0%-100%). Metrics are stored in a database.
//!
//! # Rewards data
//! Metrics are collected frequently to get accurate data about mix-nodes uptime. For each Epoch, metrics
//! average is calculated which is simply average of the all the metrics collected during previous Epoch.
//!
//! # Ephemera integration
//! Previously only single Nym-Api was submitting rewards to the smart contract.We want to have multiple
//! Nym-Api instances running for decentralization purposes.
//!
//! With Ephemera integration each Nym-Api instance tries to get reward data from other Nym-Api instances
//! and calculate the final, aggregated reward data.
//!
//! # Ephemera rewards broadcasting
//! The general idea is that each Nym-Api instance is broadcasting its reward data to other Nym-Api instances.
//! Each instance waits until it has got data from configurable threshold(rewards count threshold) of other Nym-Api instances.
//! When this threshold is reached, Nym-Api allows its local Ephemera bundle collected rewards into a block.
//! Then Ephemera runs reliable broadcast protocol to get its block signed by configurable threshold(reliable broadcast threshold)
//! of other Ephemera instances run by other Nym-Api instances.
//!
//! After Ephemera protocol is finished, each Nym-Api instance tries to submit the rewards data in its own block to smart contract.
//! Smart contract accepts only the first rewards data.
//! For better performance, Nym-Api instances could first query smart contract to check if there is already rewards data submitted.
//!
//! If Nym-Api succeeds in submitting rewards data to smart contract, it stores also this data in its database.
//! If Nym-Api fails to submit rewards data to smart contract, it should be possible it to know which Nym-Api instance succeeded.
//! So that it can query this Nym-Api instance for the final/official reward data. And also store this data in its database.
//!
//! # Rewards data verification by external parties
//!
//! Epoch reward data is available in smart contract. It should be possible to verify that this data is signed by enough Nym-Api instances.
//!
//! When a mixnode operator or auditor wants to verify that some mixnode rewards are correct, they can query any Nym-Api instance for the reward data.
//! Reward data contains the actual data and signatures of Nym-Api instances(validators).
//!

use clap::Parser;
use log::info;
use tokio::signal::unix::{signal, SignalKind};

use ephemera::configuration::Configuration;
use ephemera::logging;
use nym_api::{nym_api_ephemera::NymApi, Args};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    logging::init();

    let args = Args::parse();
    let ephemera_config = Configuration::try_load(args.ephemera_config.clone()).unwrap();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let mut nym_api = tokio::spawn(NymApi::run(args, ephemera_config, shutdown_rx));

    let mut stream_int = signal(SignalKind::interrupt()).unwrap();
    let mut stream_term = signal(SignalKind::terminate()).unwrap();

    tokio::select! {
        _ = stream_int.recv() => {
            shutdown_tx.send(()).unwrap();
        }
        _ = stream_term.recv() => {
            shutdown_tx.send(()).unwrap();
        }
        //When nym api finishes because of internal error
        _ = &mut nym_api => {}
    }

    if !nym_api.is_finished() {
        nym_api.await??;
    }

    info!("Nym-Api Ephemera simulation finished");
    Ok(())
}
