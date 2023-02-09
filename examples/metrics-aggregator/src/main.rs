use std::env;
use std::ops::{Add};
use std::sync::Arc;


use time::{Duration, OffsetDateTime};
use tokio::sync::Mutex;

use storage::db::Storage;

use crate::epoch::Epoch;
use crate::reward::RewardManager;

pub(crate) mod contract;
pub(crate) mod epoch;
pub(crate) mod metrics;
pub(crate) mod reward;
pub(crate) mod storage;

pub(crate) const NR_OF_MIX_NODES: usize = 100;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let metrics_collector_interval = Duration::seconds(5);
    log::info!(
        "Metrics collector interval: {:?}",
        metrics_collector_interval
    );
    let metrics_start_time = OffsetDateTime::now_utc().add(metrics_collector_interval);
    log::info!("Metrics start time: {:?}", metrics_start_time);

    let epoch_duration: Duration = 2_f32 * metrics_collector_interval;
    log::info!("Epoch duration: {:?}", epoch_duration);
    let epoch_start_time = metrics_start_time - Duration::seconds(1);
    log::info!("Epoch start time: {:?}", epoch_start_time);

    let epoch = Epoch::new(epoch_start_time, epoch_duration);

    let smart_contract_url = "127.0.0.1:6789".to_string();

    let db_file = env::current_dir()
        .unwrap()
        .join("metrics-aggregator.sqlite");
    log::info!("Using database file: {}", db_file.display());
    if db_file.exists() {
        log::info!("Removing previous database file: {}", db_file.display());
        std::fs::remove_file(&db_file).unwrap();
    }

    let storage = Arc::new(Mutex::new(Storage::new(db_file)));

    let smart_contract =
        contract::SmartContract::start(smart_contract_url.clone(), storage.clone(), epoch).unwrap();

    tokio::spawn(async move {
        smart_contract.await.unwrap();
    });

    let mut metrics_collector =
        metrics::MetricsCollector::new(storage.clone(), metrics_collector_interval);

    tokio::spawn(async move {
        metrics_collector.collect().await.unwrap();
    });

    let mut reward_calculator =
        RewardManager::new(storage.clone(), epoch, smart_contract_url);

    tokio::spawn(async move {
        reward_calculator
            .reward(metrics_collector_interval)
            .await
            .unwrap();
    })
    .await
    .unwrap();
}
