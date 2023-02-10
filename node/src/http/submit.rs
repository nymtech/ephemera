use actix_web::{post, web, HttpRequest, HttpResponse};

use crate::api::types::ApiSignedMessage;
use crate::api::EphemeraExternalApi;

#[utoipa::path(
    request_body = ApiSignedMessage,
    responses(
        (status = 200, description = "Send a message to an Ephemera node to be signed by Ephemera cluster")
    )
)]
#[post("/ephemera/submit_message")]
pub(crate) async fn submit_message(
    _req: HttpRequest,
    message: web::Json<ApiSignedMessage>,
    api: web::Data<EphemeraExternalApi>,
) -> HttpResponse {
    log::debug!("POST /ephemera/submit_message {}", message.request_id);

    match api.submit_signed_message(message.into_inner()).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(err) => {
            log::error!("Error submitting message: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}
