use std::time::Duration;

use async_trait::async_trait;

use crate::reward::{EpochOperations, RewardManager, V2};

pub(crate) mod aggregator;

#[async_trait]
impl EpochOperations for RewardManager<V2> {
    async fn perform_epoch_operations(&mut self) -> anyhow::Result<()> {
        //Calculate rewards for the epoch which just ended
        let rewards = self.calculate_rewards_for_previous_epoch().await?;
        let nr_of_rewards = rewards.len();

        //Submit our own rewards message.
        //It will be included in the next block(ours and/or others)
        self.send_rewards_to_ephemera(rewards).await?;

        //Assuming that next block includes our rewards message
        //This assumptions need to be "configured" by the application.
        let prev_block = self.get_last_block().await?;
        let next_height = prev_block.header.height + 1;

        //Poll next block which should include all messages from the previous epoch from almost all Nym-Api nodes
        let mut counter = 0;
        log::info!(
            "Waiting for block with height {next_height} maximum {} seconds",
            self.args.block_polling_max_attempts * self.args.block_polling_interval_seconds
        );
        loop {
            if counter > self.args.block_polling_max_attempts {
                log::error!(
                    "Block for height {next_height} is not available after {counter} attempts"
                );
                break;
            }
            tokio::select! {
                Ok(Some(block)) = self.get_block_by_height(next_height) => {
                    log::info!("Received local block with height {next_height}");
                    self.try_submit_rewards_to_contract(nr_of_rewards, block)
                        .await?;
                    break;
                }
                _ = tokio::time::sleep(Duration::from_secs(self.args.block_polling_interval_seconds)) => {
                    log::trace!("Block for height {next_height} is not available yet, waiting...");
                }
            }
            counter += 1;
        }

        // TODO: query smart contract for the nym-api which was able to submit rewards and query its block.
        // TODO: Because each Ephemera "sees" all blocks during RB then it might be safe to save them locally
        // TODO: already during RB. In case of failure of that node.

        log::info!("Finished reward calculation for previous epoch");
        Ok(())
    }
}
