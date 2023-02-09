use std::sync::Arc;


use time::{Duration, OffsetDateTime};
use tokio::sync::Mutex;


use crate::contract::{MixnodeToReward};
use crate::epoch::Epoch;
use crate::NR_OF_MIX_NODES;
use crate::storage::db::Storage;

pub(crate) struct RewardManager {
    pub(crate) storage: Arc<Mutex<Storage>>,
    pub(crate) epoch: Epoch,
    pub(crate) contract_url: String,
}

impl RewardManager {
    pub(crate) fn new(storage: Arc<Mutex<Storage>>, epoch: Epoch, contract_url: String) -> Self {
        Self {
            storage,
            epoch,
            contract_url,
        }
    }

    pub(crate) async fn reward(&mut self, _interval: Duration) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                _ = self.epoch.tick() => {
                    let start = self.epoch.current_epoch_start_time().unix_timestamp() as u64;
                    let end = self.epoch.current_epoch_end_time().unix_timestamp() as u64;

                    log::info!("Calculating rewards for interval {} - {}", start, end);
                    let rewards = self.calculate_rewards(start, end).await?;
                    log::trace!("Calculated rewards {:?}", rewards);

                    log::info!("Submitting rewards for {} mixnodes", NR_OF_MIX_NODES);
                    self.submit_rewards(rewards).await?;
                }
            }
        }
    }

    async fn calculate_rewards(
        &self,
        start: u64,
        end: u64,
    ) -> anyhow::Result<Vec<MixnodeToReward>> {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        log::info!(
            "Calculating rewards for interval {} - {}, now:{}",
            start,
            end,
            now
        );

        let mut uptimes = Vec::with_capacity(NR_OF_MIX_NODES);
        let storage = self.storage.lock().await;
        for i in 0..NR_OF_MIX_NODES {
            let reliability = storage.get_mixnode_average_reliability_in_interval(i, start, end)?;
            uptimes.push(MixnodeToReward {
                mix_id: i,
                performance: reliability.unwrap_or_default() as u8,
            });
        }
        Ok(uptimes)
    }

    async fn submit_rewards(&self, rewards: Vec<MixnodeToReward>) -> anyhow::Result<()> {
        let url = format!("http://{}/contract/submit_reward", self.contract_url);
        log::info!("Submitting rewards to {}", url);
        for reward in rewards {
            let response = reqwest::Client::new()
                .post(url.clone())
                .json(&reward)
                .send()
                .await?;
            if !response.status().is_success() {
                log::error!(
                    "Failed to submit reward for mixnode {}, status: {}",
                    reward.mix_id,
                    response.status()
                );
            }
        }
        Ok(())
    }
}
