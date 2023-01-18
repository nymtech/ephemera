use crate::broadcast_callback::SigningBroadcastCallBack;
use ephemera::config::configuration::Configuration;
use ephemera::network::client_listener::EphemeraNetworkCmdListener;
use ephemera::network::ephemera::EphemeraLauncher;
use futures::executor::block_on;

mod backend;
mod broadcast_callback;
pub mod cli;

const CONFIG_DIR: &str = "configuration";

#[tokio::main]
async fn main() {
    init_logging();

    let args = cli::parse_args();
    let settings = Configuration::try_load(CONFIG_DIR, args.config_file.as_str()).unwrap();

    let mut app = SigningBroadcastCallBack::new()
        .with_file_backend(args.signatures_file)
        .with_ws_backend(args.ws_listen_addr)
        .with_db_backend(args.db_url);

    app.start().await.unwrap();

    let ephemera = EphemeraLauncher::launch(settings, app).await;

    //Gets commands from network and sends these to ephemera
    block_on(EphemeraNetworkCmdListener::new(ephemera, args.client_listener_address).run());
}

fn init_logging() {
    if let Ok(directives) = ::std::env::var("RUST_LOG") {
        pretty_env_logger::formatted_timed_builder()
            .parse_filters(&directives)
            .format_timestamp_millis()
            .init();
    }
}