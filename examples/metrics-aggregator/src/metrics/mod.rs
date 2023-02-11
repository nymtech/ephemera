use std::sync::Arc;

use rand::Rng;
use time::{Duration, OffsetDateTime};
use tokio::sync::Mutex;

use crate::metrics::types::MixnodeResult;
use crate::storage::db::Storage;
use crate::NR_OF_MIX_NODES;

pub(crate) mod types;

pub(crate) struct MetricsCollector {
    pub(crate) storage: Arc<Mutex<Storage>>,
    pub(crate) metrics_interval: Duration,
}

impl MetricsCollector {
    pub(crate) fn new(
        storage: Arc<Mutex<Storage>>,
        metrics_interval: Duration,
    ) -> MetricsCollector {
        MetricsCollector {
            storage,
            metrics_interval,
        }
    }

    pub(crate) async fn collect(&mut self) -> anyhow::Result<()> {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            self.metrics_interval.whole_seconds() as u64,
        ));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let metrics = self.generate_metrics();
                    let mut storage = self.storage.lock().await;
                    let now = OffsetDateTime::now_utc().unix_timestamp();
                    log::info!("Submitting metrics for {} mixnodes, {}", NR_OF_MIX_NODES, now);
                    storage.submit_mixnode_statuses(now, metrics)?;
                }
            }
        }
    }

    fn generate_metrics(&self) -> Vec<MixnodeResult> {
        let mut metrics = Vec::with_capacity(NR_OF_MIX_NODES);
        let mut rng = rand::thread_rng();

        for i in 0..NR_OF_MIX_NODES {
            let reliability = rng.gen_range(1..100) as u8;
            metrics.push(MixnodeResult {
                mix_id: i as u32,
                reliability,
            });
        }
        log::trace!("Generated metrics {:?}", metrics);
        metrics
    }
}
