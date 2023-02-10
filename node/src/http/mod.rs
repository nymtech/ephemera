use actix_web::{App, HttpServer};
use actix_web::dev::Server;
use actix_web::web::Data;
use anyhow::Result;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::api::EphemeraExternalApi;
use crate::config::configuration::HttpConfig;

pub(crate) mod query;
pub(crate) mod submit;

/// Starts the HTTP server.
pub(crate) fn start(config: HttpConfig, api: EphemeraExternalApi) -> Result<Server> {
    print_startup_messages(config.clone());

    let server = HttpServer::new(move || {
        App::new()
            .app_data(Data::new(api.clone()))
            .service(query::block_by_id)
            .service(query::block_by_label)
            .service(submit::submit_message)
            .service(swagger_ui())
    })
    .bind(config.address)?
    .run();

    Ok(server)
}

/// Builds the Swagger UI.
///
/// Note that all routes you want Swagger docs for must be in the `paths` annotation.
fn swagger_ui() -> SwaggerUi {
    use crate::api::types;
    #[derive(OpenApi)]
    #[openapi(
        paths(query::block_by_id, query::block_by_label, submit::submit_message),
        components(schemas(types::ApiBlock, types::ApiSignedMessage, types::ApiSignature))
    )]
    struct ApiDoc;
    SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-doc/openapi.json", ApiDoc::openapi())
}

/// Prints messages saying which ports HTTP is running on, and some helpful pointers
/// to the Swagger UI and OpenAPI spec.
fn print_startup_messages(config: HttpConfig) {
    log::info!("Server running on {}", config.address);
    log::info!("Swagger UI: {}/swagger-ui/", config.address);
    log::info!(
        "OpenAPI spec is at: {}/api-doc/openapi.json",
        config.address
    );
}
