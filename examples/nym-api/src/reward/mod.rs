use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast::Receiver;
use tokio::sync::Mutex;

use ephemera::api::types::{ApiBlock, ApiEphemeraMessage};
use ephemera::api::{ApiError, EphemeraExternalApi};
use ephemera::crypto::{EphemeraKeypair, EphemeraPublicKey, Keypair};

use crate::contract::MixnodeToReward;
use crate::epoch::Epoch;
use crate::peers::PeerId;
use crate::reward::new::aggregator::RewardsAggregator;
use crate::storage::db::{MetricsStorageType, Storage};
use crate::{Args, HTTP_NYM_API_HEADER, NR_OF_MIX_NODES};

pub(crate) mod new;
mod old;

type MixId = usize;

pub(crate) struct V1;

pub(crate) struct V2;

pub struct EphemeraAccess {
    pub(crate) api: EphemeraExternalApi,
    pub(crate) key_pair: Keypair,
}

impl EphemeraAccess {
    pub(crate) fn new(api: EphemeraExternalApi, key_pair: Keypair) -> Self {
        Self { api, key_pair }
    }
}

#[async_trait]
pub(crate) trait EpochOperations {
    async fn perform_epoch_operations(&mut self) -> anyhow::Result<()>;
}

pub(crate) struct RewardManager<V> {
    pub storage: Arc<Mutex<Storage<MetricsStorageType>>>,
    pub epoch: Epoch,
    pub args: Args,
    pub version: PhantomData<V>,
    pub ephemera_access: Option<EphemeraAccess>,
    aggregator: Option<RewardsAggregator>,
}

impl<V> RewardManager<V>
where
    Self: EpochOperations,
{
    pub(crate) fn new(
        storage: Arc<Mutex<Storage<MetricsStorageType>>>,
        args: Args,
        ephemera_access: Option<EphemeraAccess>,
        aggregator: Option<RewardsAggregator>,
        epoch: Epoch,
    ) -> Self {
        log::info!(
            "Starting RewardManager with epoch nr {}",
            epoch.current_epoch_numer()
        );
        Self {
            storage,
            epoch,
            args,
            version: Default::default(),
            ephemera_access,
            aggregator,
        }
    }

    pub(crate) async fn start(mut self, mut receiver: Receiver<()>) {
        loop {
            tokio::select! {
                _ = receiver.recv() => {
                    log::info!("Shutting down reward manager");
                    break;
                }
                _ =  self.epoch.wait_epoch_end() => {
                    log::info!("Rewarding epoch {} ...", self.epoch.current_epoch_numer());
                    if let Err(err) = self.perform_epoch_operations().await {
                        log::error!("Reward calculator failed: {}", err);
                    }
                }
            }
        }
    }

    pub(crate) async fn calculate_rewards_for_previous_epoch(
        &self,
    ) -> anyhow::Result<Vec<MixnodeToReward>> {
        let start = self.epoch.current_epoch_start_time().timestamp() as u64;
        let end = self.epoch.current_epoch_end_time().timestamp() as u64;
        log::info!("Calculating rewards for interval {} - {}", start, end);

        let mix_nodes = self.get_mix_nodes_to_reward();
        log::debug!("Mix nodes to reward: {:?}", mix_nodes);

        let storage = self.storage.lock().await;

        let mut uptimes = Vec::with_capacity(NR_OF_MIX_NODES);
        for mix_id in mix_nodes {
            let reliability = storage.get_mixnode_average_reliability(mix_id, start, end)?;
            uptimes.push(MixnodeToReward::new(
                mix_id,
                reliability.unwrap_or_default() as u8,
            ));
        }

        Ok(uptimes)
    }

    pub(crate) async fn submit_rewards_to_contract(
        &self,
        rewards: Vec<MixnodeToReward>,
    ) -> anyhow::Result<()> {
        let url = format!(
            "http://{}/contract/submit_rewards",
            self.args.smart_contract_url
        );
        log::info!("Submitting rewards to {}", url);
        let response = reqwest::Client::new()
            .post(url.clone())
            .header(HTTP_NYM_API_HEADER, self.args.nym_api_id.clone())
            .json(&rewards)
            .send()
            .await?;

        log::info!("Response from contract: {:?}", response);

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to submit rewards to contract with status {}",
                response.status()
            ));
        }
        Ok(())
    }

    pub(crate) async fn get_last_block(&self) -> Result<ApiBlock, ApiError> {
        let access = self
            .ephemera_access
            .as_ref()
            .expect("Ephemera access not set");
        let block = access.api.get_last_block().await?;
        Ok(block)
    }

    pub(crate) async fn get_block_by_height(
        &self,
        height: u64,
    ) -> Result<Option<ApiBlock>, ApiError> {
        let access = self
            .ephemera_access
            .as_ref()
            .expect("Ephemera access not set");
        let block = access.api.get_block_by_height(height).await?;
        Ok(block)
    }

    pub(crate) async fn store_in_dht(&self, epoch_id: u64) -> anyhow::Result<()> {
        log::info!("Storing ourselves as winner in DHT for epoch id: {epoch_id:?}");

        let access = self
            .ephemera_access
            .as_ref()
            .expect("Ephemera access not set");

        let key = format!("epoch_id_{epoch_id}").into_bytes();

        let keypair = &access.key_pair;
        let value = keypair.public_key().to_base58();

        access.api.store_in_dht(key, value.into_bytes()).await?;
        log::info!("Sent store request to DHT");
        Ok(())
    }

    pub(crate) async fn query_dht(&self, epoch_id: u64) -> anyhow::Result<Option<String>> {
        let access = self
            .ephemera_access
            .as_ref()
            .expect("Ephemera access not set");

        let key = format!("epoch_id_{epoch_id}").into_bytes();

        match access.api.query_dht(key).await? {
            None => {
                log::info!("No winner found for epoch id from DHT: {epoch_id:?}");
                Ok(None)
            }
            Some((_, peer_id)) => {
                let peer_id = PeerId::from_utf8(peer_id).expect("Invalid peer id");
                log::info!("Winner found for epoch id from DHT: {epoch_id:?} - {peer_id:?}");
                Ok(Some(peer_id))
            }
        }
    }

    pub(crate) async fn send_rewards_to_ephemera(
        &self,
        rewards: Vec<MixnodeToReward>,
    ) -> anyhow::Result<()> {
        let ephemera_msg = self.create_ephemera_message(rewards)?;
        log::debug!("Sending rewards to ephemera: {:?}", ephemera_msg);

        let access = self
            .ephemera_access
            .as_ref()
            .expect("Ephemera access not set");

        access.api.send_ephemera_message(ephemera_msg).await?;
        Ok(())
    }

    fn create_ephemera_message(
        &self,
        rewards: Vec<MixnodeToReward>,
    ) -> anyhow::Result<ApiEphemeraMessage> {
        let keypair = &self
            .ephemera_access
            .as_ref()
            .expect("Ephemera access not set")
            .key_pair;

        let data = serde_json::to_vec(&rewards)?;

        let signature = keypair.sign(&data)?;
        let pub_key = keypair.public_key();

        let api_signature = ephemera::api::types::ApiCertificate::new(signature, pub_key);

        let epoch_nr = self.epoch.current_epoch_numer().to_string();
        let api_signed_message = ApiEphemeraMessage::new(data, api_signature, epoch_nr);

        Ok(api_signed_message)
    }

    //By current assumption, all nodes will try submit their aggregated rewards
    //and contract will reject all but first one.
    async fn try_submit_rewards_to_contract(
        &mut self,
        nr_of_rewards: usize,
        block: ApiBlock,
    ) -> anyhow::Result<()> {
        log::info!(
            "Calculating aggregated rewards from block with height: {:?}",
            block.header.height
        );
        let mut mix_node_rewards = vec![];

        for message in block.messages {
            log::trace!("Message: {}", message);
            let mix_node_reward: Vec<MixnodeToReward> = serde_json::from_slice(&message.data)?;
            mix_node_rewards.push(mix_node_reward);
        }

        let aggregated_rewards = self.aggregator().aggregate(mix_node_rewards);
        log::debug!("Aggregated rewards: {:?}", aggregated_rewards);

        self.submit_rewards_to_contract(aggregated_rewards).await?;

        self.storage
            .lock()
            .await
            .save_rewarding_results(self.epoch.current_epoch_numer(), nr_of_rewards)?;
        log::info!(
            "Saved rewarding results for epoch: {:?}",
            self.epoch.current_epoch_numer()
        );

        Ok(())
    }

    fn aggregator(&self) -> &RewardsAggregator {
        self.aggregator.as_ref().expect("Aggregator not set")
    }

    fn get_mix_nodes_to_reward(&self) -> Vec<MixId> {
        (0..NR_OF_MIX_NODES).collect::<Vec<_>>()
    }
}
