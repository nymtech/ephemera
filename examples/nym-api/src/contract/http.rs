use actix_web::{get, post, web, HttpRequest, HttpResponse};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::contract::{MixnodeToReward, SmartContract};
use crate::HTTP_NYM_API_HEADER;

#[post("/contract/submit_rewards")]
pub async fn submit_reward(
    req: HttpRequest,
    message: web::Json<Vec<MixnodeToReward>>,
    contract: web::Data<Arc<Mutex<SmartContract>>>,
) -> HttpResponse {
    log::debug!("POST /contract/submit_reward {:?}", message);

    let nym_api_id = req
        .headers()
        .get(HTTP_NYM_API_HEADER)
        .unwrap()
        .to_str()
        .unwrap();
    let rewards = message.into_inner();

    log::debug!(
        "Received {} reward submissions from {nym_api_id}",
        rewards.len(),
    );
    log::debug!("Reward info {:?}", rewards);

    match contract
        .lock()
        .await
        .submit_mix_rewards(nym_api_id, rewards)
        .await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(err) => {
            log::error!("Error submitting message: {}", err);
            HttpResponse::Conflict().json(err.to_string())
        }
    }
}

#[get("/contract/epoch")]
pub async fn get_epoch(contract: web::Data<Arc<Mutex<SmartContract>>>) -> HttpResponse {
    match contract.lock().await.get_epoch_from_db().await {
        Ok(epoch) => {
            log::debug!("GET /contract/epoch {:?}", epoch);
            HttpResponse::Ok().json(epoch)
        }
        Err(err) => {
            log::error!("Error getting epoch: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}
