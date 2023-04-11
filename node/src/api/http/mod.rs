use actix_web::{dev::Server, http::KeepAlive, web::Data, App, HttpServer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::api::EphemeraExternalApi;
use crate::core::builder::NodeInfo;

pub(crate) mod client;
pub(crate) mod query;
pub(crate) mod submit;

/// Starts the HTTP server.
pub(crate) fn init(node_info: &NodeInfo, api: EphemeraExternalApi) -> anyhow::Result<Server> {
    print_startup_messages(node_info);

    let server = HttpServer::new(move || {
        App::new()
            .app_data(Data::new(api.clone()))
            .service(query::health)
            .service(query::block_by_hash)
            .service(query::block_certificates)
            .service(query::block_by_height)
            .service(query::last_block)
            .service(query::get_node_config)
            .service(query::query_dht)
            .service(submit::submit_message)
            .service(submit::store_in_dht)
            .service(swagger_ui())
    })
    .keep_alive(KeepAlive::Os)
    .bind((node_info.ip.as_str(), node_info.initial_config.http.port))?
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
            query::block_by_hash,
            query::block_certificates,
            query::block_by_height,
            query::last_block,
            query::get_node_config,
            query::query_dht,
            submit::submit_message,
            submit::store_in_dht,
        ),
        components(schemas(
            types::ApiBlock,
            types::ApiEphemeraMessage,
            types::ApiCertificate,
            types::Health,
            types::ApiEphemeraConfig,
            types::ApiDhtStoreRequest,
            types::ApiDhtQueryRequest,
            types::ApiDhtQueryResponse,
        ))
    )]
    struct ApiDoc;
    SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-doc/openapi.json", ApiDoc::openapi())
}

/// Prints messages saying which ports HTTP is running on, and some helpful pointers
/// to the Swagger UI and OpenAPI spec.
fn print_startup_messages(info: &NodeInfo) {
    let http_root = info.api_address_http();
    log::info!("Server running on {}", http_root);
    log::info!("Swagger UI: {}/swagger-ui/", http_root);
    log::info!("OpenAPI spec is at: {}/api-doc/openapi.json", http_root);
}
