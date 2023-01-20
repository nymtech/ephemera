use actix_web::{get, HttpResponse, Responder, web};
use crate::api::queries::MessagesApi;

#[utoipa::path(
responses(
(status = 200, description = "GET message by request id")
)
)]
#[get("/ephemera/message/{id}")]
pub(crate) async fn message_by_id(id: web::Path<String>, api: web::Data<MessagesApi>) -> impl Responder {
    let id = id.into_inner();
    api.get_message_by_request_id(id)
        .map(|msg| HttpResponse::Ok().json(msg))
        .unwrap_or_else(|err| HttpResponse::InternalServerError().body(err.to_string()))
}

#[utoipa::path(
responses(
(status = 200, description = "GET message by custom message id, for example 'epoch-100', depending on the application")
)
)]
#[get("/ephemera/message/custom_id/{id}")]
pub(crate) async fn message_by_custom_id(id: web::Path<String>, api: web::Data<MessagesApi>) -> impl Responder {
    let id = id.into_inner();
    api.get_message_by_custom_message_id(id)
        .map(|msg| HttpResponse::Ok().json(msg))
        .unwrap_or_else(|err| HttpResponse::InternalServerError().body(err.to_string()))
}