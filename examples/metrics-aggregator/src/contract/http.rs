use actix_web::{post, web, HttpRequest, HttpResponse};

use crate::contract::{MixnodeToReward, SmartContract};

#[post("/contract/submit_reward")]
pub(crate) async fn submit_reward(
    _req: HttpRequest,
    message: web::Json<MixnodeToReward>,
    contract: web::Data<SmartContract>,
) -> HttpResponse {
    log::trace!("POST /contract/submit_reward {:?}", message);

    match contract.submit_mix_reward(message.into_inner()).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(err) => {
            log::error!("Error submitting message: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}
