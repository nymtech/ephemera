use std::sync::Arc;

use actix_web::{App, HttpServer};
use actix_web::dev::Server;
use actix_web::web::Data;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::Mutex;




use crate::contract::http::submit_reward;
use crate::epoch::Epoch;

use crate::storage::db::Storage;

pub(crate) mod http;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub(crate) struct MixnodeToReward {
    pub(crate) mix_id: usize,
    pub(crate) performance: u8,
}

#[derive(Clone)]
pub(crate) struct SmartContract {
    pub(crate) storage: Arc<Mutex<Storage>>,
    pub(crate) epoch: Epoch,
}

impl SmartContract {
    pub(crate) fn new(storage: Arc<Mutex<Storage>>, epoch: Epoch) -> Self {
        Self { storage, epoch }
    }

    pub(crate) fn start(
        url: String,
        storage: Arc<Mutex<Storage>>,
        epoch: Epoch,
    ) -> anyhow::Result<Server> {
        let server = HttpServer::new(move || {
            let smart_contract = SmartContract::new(storage.clone(), epoch);
            App::new()
                .app_data(Data::new(smart_contract))
                .service(submit_reward)
        })
        .bind(url)?
        .run();

        Ok(server)
    }

    pub(crate) async fn submit_mix_reward(
        &self,
        mixnode_to_reward: MixnodeToReward,
    ) -> anyhow::Result<()> {
        let mut storage = self.storage.lock().await;
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let epoch_id = self.epoch.current_epoch_numer();

        log::info!(
            "Submitting reward for mixnode {} in epoch {} at {}",
            mixnode_to_reward.mix_id,
            epoch_id,
            now
        );
        storage.contract_submit_mixnode_reward(epoch_id, now, mixnode_to_reward)?;
        Ok(())
    }
}
