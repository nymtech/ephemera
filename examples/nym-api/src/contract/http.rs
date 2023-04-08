use std::sync::Arc;

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use lazy_static::lazy_static;
use tokio::sync::Mutex;

use ephemera::crypto::EphemeraPublicKey;

use crate::contract::{MixnodeToReward, SmartContract};
use crate::nym_api_ephemera::peer_discovery::NymPeerInfo;
use crate::HTTP_NYM_API_HEADER;

lazy_static! {
    static ref HTTP_CLIENT: reqwest::Client = reqwest::Client::builder().build().unwrap();
}

#[post("/contract/submit_rewards")]
pub(crate) async fn submit_reward(
    req: HttpRequest,
    message: web::Json<Vec<MixnodeToReward>>,
    contract: web::Data<Arc<Mutex<SmartContract>>>,
) -> HttpResponse {
    log::info!("POST /contract/submit_reward {:?}", message);

    let nym_api_id = req
        .headers()
        .get(HTTP_NYM_API_HEADER)
        .unwrap()
        .to_str()
        .unwrap();
    let rewards = message.into_inner();

    log::info!(
        "Received {} reward submissions from {nym_api_id}",
        rewards.len(),
    );
    log::info!("Reward info {:?}", rewards);

    match contract
        .lock()
        .await
        .submit_mix_rewards(nym_api_id, rewards)
        .await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(err) => {
            log::error!(
                "Failed to submit message, only first Nym-Api succeeds: {}",
                err
            );
            HttpResponse::Conflict().json(err.to_string())
        }
    }
}

#[get("/contract/epoch")]
pub(crate) async fn get_epoch(contract: web::Data<Arc<Mutex<SmartContract>>>) -> HttpResponse {
    match contract.lock().await.get_epoch_from_db().await {
        Ok(epoch) => {
            log::info!("GET /contract/epoch {:?}", epoch);
            HttpResponse::Ok().json(epoch)
        }
        Err(err) => {
            log::error!("Error getting epoch: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/contract/peer_info")]
pub(crate) async fn get_nym_apis(contract: web::Data<Arc<Mutex<SmartContract>>>) -> HttpResponse {
    log::info!("GET /contract/peer_info");

    let mut peers: Vec<NymPeerInfo> = vec![];
    for (i, (peer_id, peer)) in contract
        .lock()
        .await
        .peer_info
        .peers
        .clone()
        .into_iter()
        .enumerate()
    {
        if !ping_health(i).await {
            log::info!("Skipping peer {} as it seems down", peer_id);
            continue;
        }

        peers.push(NymPeerInfo {
            name: peer_id.to_string(),
            address: peer.address,
            pub_key: peer.public_key.to_base58(),
        });
    }

    log::info!("Found {} peers", peers.len());
    for peer in peers.iter() {
        log::info!("Peer address: {:?}", peer.address);
    }

    HttpResponse::Ok().json(peers)
}

async fn ping_health(node_id: usize) -> bool {
    let url = format!("http://127.0.0.1:700{node_id}/ephemera/node/health",);
    log::info!("Pinging health endpoint at {}", url);
    let response = HTTP_CLIENT.get(url).send().await;

    if response.is_err() {
        return false;
    }

    response.unwrap().status().is_success()
}
