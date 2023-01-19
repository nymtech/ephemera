pub(crate) mod hello;

use actix_web::{App, HttpServer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

/// Starts the HTTP server.
pub(crate) async fn start(port: u16) {
    print_startup_messages(port);

    HttpServer::new(|| {
        App::new()
            .service(hello::index)
            .service(hello::there)
            .service(swagger_ui())
    })
    .bind(("127.0.0.1", port.clone()))
    .expect(&format!("Ephemera's HTTP server couldn't bind to port {}", port))
    .run()
    .await
    .unwrap()
}

/// Builds the Swagger UI.
///
/// Note that all routes you want Swagger docs for must be in the `paths` annotation.
fn swagger_ui() -> SwaggerUi {
    #[derive(OpenApi)]
    #[openapi(paths(hello::index, hello::there))]
    struct ApiDoc;

    SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-doc/openapi.json", ApiDoc::openapi())
}

/// Prints messages saying which ports HTTP is running on, and some helpful pointers
/// to the Swagger UI and OpenAPI spec.
fn print_startup_messages(port: u16) {
    println!("Server running on http://127.0.0.1:{}", port);
    println!("Swagger UI: http://localhost:{}/swagger-ui/", port);
    println!(
        "OpenAPI spec is at: http://localhost:{}/api-doc/openapi.json",
        port
    );
}
