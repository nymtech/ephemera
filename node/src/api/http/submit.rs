use actix_web::{post, web, HttpRequest, HttpResponse};

use crate::api::types::{ApiDhtStoreRequest, ApiEphemeraMessage};
use crate::api::{ApiError, EphemeraExternalApi};

#[utoipa::path(
request_body = ApiSignedMessage,
responses(
(status = 200, description = "Send a message to an Ephemera node which will be broadcast to the network"),
(status = 500, description = "Server failed to process request")),
params(("message", description = "Message to send"))
)]
#[post("/ephemera/broadcast/submit_message")]
pub(crate) async fn submit_message(
    _req: HttpRequest,
    message: web::Json<ApiEphemeraMessage>,
    api: web::Data<EphemeraExternalApi>,
) -> HttpResponse {
    log::debug!("POST /ephemera/broadcast/submit_message {:?}", message);

    match api.send_ephemera_message(message.into_inner()).await {
        Ok(_) => HttpResponse::Ok().json("Message submitted"),
        Err(err) => match err {
            ApiError::DuplicateMessage => {
                log::debug!("Message already submitted {err:?}");
                HttpResponse::BadRequest().json("Message already submitted")
            }
            _ => {
                log::error!("Error submitting message: {}", err);
                HttpResponse::InternalServerError().json("Server failed to process request")
            }
        },
    }
}

#[utoipa::path(
request_body = ApiDhtStoreRequest,
responses(
(status = 200, description = "Request to store a value in the DHT"),
(status = 500, description = "Server failed to process request")),
params(
("request", description = "Dht store request")
)
)]
#[post("/ephemera/dht/store")]
pub(crate) async fn store_in_dht(
    request: web::Json<ApiDhtStoreRequest>,
    api: web::Data<EphemeraExternalApi>,
) -> HttpResponse {
    let request = request.into_inner();
    log::debug!("POST /ephemera/dht/store {:?}", request);

    let key = request.key();
    let value = request.value();

    match api.store_in_dht(key, value).await {
        Ok(_) => HttpResponse::Ok().json("Store request submitted"),
        Err(err) => {
            log::error!("Error storing in dht: {}", err);
            HttpResponse::InternalServerError().json("Server failed to process request")
        }
    }
}
