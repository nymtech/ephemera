use actix_web::{get, web, HttpResponse, Responder};

use crate::api::EphemeraExternalApi;

#[utoipa::path(
responses(
(status = 200, description = "GET block by id")),
params(("id", description = "Block id")),
)]
#[get("/ephemera/block/{id}")]
pub(crate) async fn block_by_id(
    id: web::Path<String>,
    api: web::Data<EphemeraExternalApi>,
) -> impl Responder {
    log::debug!("GET /ephemera/block/{id}",);

    match api.get_block_by_id(id.into_inner()).await {
        Ok(Some(block)) => HttpResponse::Ok().json(block),
        Ok(_) => HttpResponse::NotFound().json("Block not found"),
        Err(err) => {
            log::error!("Failed to get block by id: {err}",);
            HttpResponse::InternalServerError().json("Server failed to process request")
        }
    }
}

#[utoipa::path(
responses(
(status = 200, description = "Get block signatures")),
params(("id", description = "Block id")),
)]
#[get("/ephemera/block/signatures/{id}")]
pub(crate) async fn block_signatures(
    id: web::Path<String>,
    api: web::Data<EphemeraExternalApi>,
) -> impl Responder {
    let id = id.into_inner();
    log::debug!("GET /ephemera/block/signatures/{id}");

    match api.get_block_signatures(id.clone()).await {
        Ok(Some(signatures)) => HttpResponse::Ok().json(signatures),
        Ok(_) => HttpResponse::NotFound().json("Signatures not found"),
        Err(err) => {
            log::error!("Failed to get signatures {err}",);
            HttpResponse::InternalServerError().json("Server failed to process request")
        }
    }
}

#[utoipa::path(
responses(
(status = 200, description = "Get block by height")),
params(("height", description = "Block height")),
)]
#[get("/ephemera/block/height/{height}")]
pub(crate) async fn block_by_height(
    height: web::Path<u64>,
    api: web::Data<EphemeraExternalApi>,
) -> impl Responder {
    log::debug!("GET /ephemera/block/height/{height}");

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
(status = 200, description = "Get last block")),
)]
//Need to use plural(blocks), otherwise overlaps with block_by_id route
#[get("/ephemera/blocks/last")]
pub(crate) async fn last_block(api: web::Data<EphemeraExternalApi>) -> impl Responder {
    log::debug!("GET /ephemera/block/last");

    let response = match api.get_last_block().await {
        Ok(block) => {
            log::debug!("Found block: {:?}", block);
            HttpResponse::Ok().json(block)
        }
        Err(err) => {
            log::error!("Failed to get block {err}",);
            HttpResponse::InternalServerError().json("Server failed to process request")
        }
    };
    log::info!("Response: {:?}", response);
    response
}
