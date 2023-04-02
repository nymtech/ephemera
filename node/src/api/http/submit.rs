use actix_web::{post, web, HttpRequest, HttpResponse};

use crate::api::types::ApiEphemeraMessage;
use crate::api::EphemeraExternalApi;

#[utoipa::path(
request_body = ApiSignedMessage,
responses(
(status = 200, description = "Send a message to an Ephemera node which will be broadcast to the network"),
(status = 500, description = "Server failed to process request")),
params(("message", description = "Message to send"))
)]
#[post("/ephemera/submit_message")]
pub(crate) async fn submit_message(
    _req: HttpRequest,
    message: web::Json<ApiEphemeraMessage>,
    api: web::Data<EphemeraExternalApi>,
) -> HttpResponse {
    log::debug!("POST /ephemera/submit_message {:?}", message);

    match api.send_ephemera_message(message.into_inner()).await {
        Ok(_) => HttpResponse::Ok().json("Message submitted"),
        Err(err) => {
            log::error!("Error submitting message: {}", err);
            HttpResponse::InternalServerError().json("Server failed to process request")
        }
    }
}
