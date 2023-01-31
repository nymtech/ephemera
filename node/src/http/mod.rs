//! An HTTP server allowing the node to be queried. Includes Swagger documentation for the API.

pub(crate) mod query;
pub(crate) mod send;

use crate::config::configuration::{Configuration, HttpConfig};
use actix_web::dev::Server;
use actix_web::web::Data;
use actix_web::{App, HttpServer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::api::queries::MessagesQueryApi;
use crate::api::send::MessageSendApi;
use anyhow::Result;
use tokio::sync::Mutex;

/// Starts the HTTP server.
pub(crate) fn start(config: Configuration, incoming_msg_api: MessageSendApi) -> Result<Server> {
    print_startup_messages(config.http_config.clone());

    let server = HttpServer::new(move || {
        let query_api = MessagesQueryApi::new(config.db_config.clone());
        App::new()
            .app_data(Data::new(query_api))
            .app_data(Data::new(Mutex::new(incoming_msg_api.clone())))
            .service(query::message_by_id)
            .service(query::message_by_custom_id)
            .service(send::send_message)
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
    #[openapi(
        paths(query::message_by_id, query::message_by_custom_id, send::send_message),
        components(schemas(send::MessageSigningRequest,))
    )]

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
