use crate::api::send::MessageSendApi;
use crate::broadcast_protocol::{pre_prepare_msg, EphemeraSigningRequest};


use actix_web::{post, web, HttpRequest, HttpResponse};
use tokio::sync::Mutex;
use serde_derive::{Deserialize};
use utoipa::{ToSchema};

#[derive(Deserialize, Debug, Clone, ToSchema)]
pub struct MessageSigningRequest {
    pub request_id: String,
    pub custom_message_id: String,
    pub message: Vec<u8>,
}

#[utoipa::path(
    request_body = MessageSigningRequest,
    responses(
        (status = 200, description = "Send a message to an Ephemera node to be signed by Ephemera cluster")
    )
)]
#[post("/ephemera/send_message")]
pub(crate) async fn send_message(
    req: HttpRequest,
    message: web::Json<MessageSigningRequest>,
    api: web::Data<Mutex<MessageSendApi>>,
) -> HttpResponse {
    let origin = req.peer_addr().unwrap().ip().to_string();
    let rb_msg = pre_prepare_msg(
        origin.clone(),
        message.custom_message_id.clone(),
        message.message.clone(),
    );

    let msg_id = rb_msg.id.clone();
    let eph_req = EphemeraSigningRequest::new(origin, rb_msg);

    api.lock().await.send_message(eph_req).await;
    HttpResponse::Ok().body(msg_id)
}
