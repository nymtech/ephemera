use actix_web::{get, web, HttpResponse, Responder};

use crate::api::types::Health;
use crate::api::EphemeraExternalApi;

#[utoipa::path(
responses(
(status = 200, description = "Endpoint to check if the server is running")),
)]
#[get("/ephemera/node/health")]
pub(crate) async fn health() -> impl Responder {
    log::debug!("GET /ephemera/node/health");
    HttpResponse::Ok().json(Health {
        status: "OK".to_string(),
    })
}

#[utoipa::path(
responses(
(status = 200, description = "GET block by hash"),
(status = 404, description = "Block not found"),
(status = 500, description = "Server failed to process request")),
params(("hash", description = "Block hash")),
)]
#[get("/ephemera/broadcast/block/{hash}")]
pub(crate) async fn block_by_hash(
    hash: web::Path<String>,
    api: web::Data<EphemeraExternalApi>,
) -> impl Responder {
    log::debug!("GET /ephemera/broadcast/block/{hash}",);

    match api.get_block_by_id(hash.into_inner()).await {
        Ok(Some(block)) => HttpResponse::Ok().json(block),
        Ok(_) => HttpResponse::NotFound().json("Block not found"),
        Err(err) => {
            log::error!("Failed to get block by hash: {err}",);
            HttpResponse::InternalServerError().json("Server failed to process request")
        }
    }
}

#[utoipa::path(
responses(
(status = 200, description = "Get block signatures"),
(status = 404, description = "Certificates not found"),
(status = 500, description = "Server failed to process request")),
params(("hash", description = "Block hash")),
)]
#[get("/ephemera/broadcast/block/certificates/{hash}")]
pub(crate) async fn block_certificates(
    hash: web::Path<String>,
    api: web::Data<EphemeraExternalApi>,
) -> impl Responder {
    let id = hash.into_inner();
    log::debug!("GET /ephemera/broadcast/block/certificates/{id}");

    match api.get_block_certificates(id.clone()).await {
        Ok(Some(signatures)) => HttpResponse::Ok().json(signatures),
        Ok(_) => HttpResponse::NotFound().json("Certificates not found"),
        Err(err) => {
            log::error!("Failed to get signatures {err}",);
            HttpResponse::InternalServerError().json("Server failed to process request")
        }
    }
}

#[utoipa::path(
responses(
(status = 200, description = "Get block by height"),
(status = 404, description = "Block not found"),
(status = 500, description = "Server failed to process request")),
params(("height", description = "Block height")),
)]
#[get("/ephemera/broadcast/block/height/{height}")]
pub(crate) async fn block_by_height(
    height: web::Path<u64>,
    api: web::Data<EphemeraExternalApi>,
) -> impl Responder {
    log::debug!("GET /ephemera/broadcast/block/height/{height}");

    match api.get_block_by_height(height.into_inner()).await {
        Ok(Some(block)) => HttpResponse::Ok().json(block),
        Ok(_) => HttpResponse::NotFound().json("Block not found"),
        Err(err) => {
            log::error!("Failed to get block {err}",);
            HttpResponse::InternalServerError().json("Server failed to process request")
        }
    }
}

#[utoipa::path(
responses(
(status = 200, description = "Get last block"),
(status = 500, description = "Server failed to process request")),
)]
//Need to use plural(blocks), otherwise overlaps with block_by_id route
#[get("/ephemera/broadcast/blocks/last")]
pub(crate) async fn last_block(api: web::Data<EphemeraExternalApi>) -> impl Responder {
    log::debug!("GET /ephemera/broadcast/blocks/last");

    match api.get_last_block().await {
        Ok(block) => HttpResponse::Ok().json(block),
        Err(err) => {
            log::error!("Failed to get block {err}",);
            HttpResponse::InternalServerError().json("Server failed to process request")
        }
    }
}

#[utoipa::path(
responses(
(status = 200, description = "Get node config"),
(status = 500, description = "Server failed to process request")),
)]
#[get("/ephemera/node/config")]
pub(crate) async fn get_node_config(api: web::Data<EphemeraExternalApi>) -> impl Responder {
    log::debug!("GET /ephemera/node/config");

    match api.get_node_config().await {
        Ok(config) => HttpResponse::Ok().json(config),
        Err(err) => {
            log::error!("Failed to get node config {err}",);
            HttpResponse::InternalServerError().json("Server failed to process request")
        }
    }
}
