use actix_web::{dev::Server, web::Data, App, HttpServer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::api::EphemeraExternalApi;
use crate::config::HttpConfig;

pub(crate) mod query;
pub(crate) mod submit;

/// Starts the HTTP server.
pub(crate) fn init(config: HttpConfig, api: EphemeraExternalApi) -> anyhow::Result<Server> {
    print_startup_messages(config.clone());

    let server = HttpServer::new(move || {
        App::new()
            .app_data(Data::new(api.clone()))
            .service(query::health)
            .service(query::block_by_id)
            .service(query::block_signatures)
            .service(query::block_by_height)
            .service(query::last_block)
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
        paths(
            query::health,
            query::block_by_id,
            query::block_signatures,
            query::block_by_height,
            query::last_block,
            submit::submit_message
        ),
        components(schemas(types::ApiBlock, types::ApiEphemeraMessage, types::ApiCertificate))
    )]
    struct ApiDoc;
    SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-doc/openapi.json", ApiDoc::openapi())
}

/// Prints messages saying which ports HTTP is running on, and some helpful pointers
/// to the Swagger UI and OpenAPI spec.
fn print_startup_messages(config: HttpConfig) {
    log::info!("Server running on http://{}", config.address);
    log::info!("Swagger UI: http://{}/swagger-ui/", config.address);
    log::info!(
        "OpenAPI spec is at: http://{}/api-doc/openapi.json",
        config.address
    );
}
