use actix_web::{get, HttpResponse, Responder};

/// Get hello world.
///
#[utoipa::path(
    responses(
        (status = 200, description = "Hello world")
    )
)]
#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

/// Get hello there.
///
#[utoipa::path(
    responses(
        (status = 200, description = "Hello there")
    )
)]
#[get("/hello/there")]
async fn there() -> impl Responder {
    HttpResponse::Ok().body("Hello there!")
}
