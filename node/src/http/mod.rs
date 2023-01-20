pub(crate) mod message;

use crate::config::configuration::{Configuration, HttpConfig};
use actix_web::dev::Server;
use actix_web::{App, HttpServer};
use actix_web::web::Data;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use anyhow::Result;
use crate::api::queries::MessagesApi;

/// Starts the HTTP server.
pub(crate) fn start(config: Configuration) -> Result<Server> {
    print_startup_messages(config.http_config.clone());

    let server = HttpServer::new(move || {
        let api = MessagesApi::new(config.db_config.clone());
        App::new()
            .app_data(Data::new(api))
            .service(message::message_by_id)
            .service(message::message_by_custom_id)
            .service(swagger_ui())
    })
        .bind(config.http_config.address)?
        .run();
    Ok(server)
}

/// Builds the Swagger UI.
///
/// Note that all routes you want Swagger docs for must be in the `paths` annotation.
fn swagger_ui() -> SwaggerUi {
    #[derive(OpenApi)]
    #[openapi(paths(message::message_by_id, message::message_by_custom_id))]
    struct ApiDoc;

    SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-doc/openapi.json", ApiDoc::openapi())
}

/// Prints messages saying which ports HTTP is running on, and some helpful pointers
/// to the Swagger UI and OpenAPI spec.
fn print_startup_messages(config: HttpConfig) {
    log::info!("Server running on {}", config.address);
    log::info!("Swagger UI: {}/swagger-ui/", config.address);
    log::info!("OpenAPI spec is at: {}/api-doc/openapi.json", config.address);
}
