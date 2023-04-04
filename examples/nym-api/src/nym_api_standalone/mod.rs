use std::sync::Arc;

use tokio::sync::Mutex;

use metrics::MetricsCollector;

use crate::epoch::Epoch;
use crate::reward::RewardManager;
use crate::reward::{EpochOperations, V1};
use crate::storage::db::Storage;
use crate::{metrics, Args};

mod migrations {
    use refinery::embed_migrations;

    embed_migrations!("migrations/rewardsnew");
}

pub struct NymApi {}

impl NymApi {
    pub async fn run(args: Args) {
        let storage = Arc::new(Mutex::new(Storage::init(
            args.metrics_db_path.clone(),
            migrations::migrations::runner(),
        )));

        let mut metrics =
            MetricsCollector::new(storage.clone(), args.metrics_collector_interval_seconds);

        let epoch = Epoch::request_epoch(args.smart_contract_url.clone()).await;

        let mut reward: RewardManager<V1> = RewardManager::new(storage, args, None, None, epoch);

        loop {
            tokio::select! {
                _ = metrics.interval.tick() => {
                    if let Err(err) = metrics.collect().await {
                        log::error!("Metrics collector failed: {}", err);
                    }
                }
                _ = reward.epoch.wait_epoch_end() => {
                    log::info!("Rewarding epoch ...");
                    if let Err(err) = reward.perform_epoch_operations().await {
                        log::error!("Reward calculator failed: {}", err);
                    }
                }
            }
        }
    }
}
