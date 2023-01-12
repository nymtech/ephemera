mod backend;
mod broadcast_callback;
pub mod cli;

use crate::broadcast_callback::SigningBroadcastCallBack;

use ephemera::config::configuration::Configuration;
use ephemera::network::client_listener::EphemeraNetworkCmdListener;
use ephemera::network::ephemera::EphemeraLauncher;
use futures::executor::block_on;
use std::env;

const CONFIG_DIR: &str = "configuration";

#[tokio::main]
async fn main() {
    if !env::vars().any(|(k, _)| k == "RUST_LOG") {
        env::set_var("RUST_LOG", "info");
    }
    pretty_env_logger::init();

    let args = cli::parse_args();
    let settings = Configuration::try_load(CONFIG_DIR, args.config_file.as_str()).unwrap();

    let mut app = SigningBroadcastCallBack::new()
        .with_file_backend(args.signatures_file)
        .with_ws_backend(args.ws_listen_addr);

    app.start().await.unwrap();

    let ephemera = EphemeraLauncher::launch(settings, app).await;

    //Gets commands from network and sends these to ephemera
    block_on(EphemeraNetworkCmdListener::new(ephemera, args.client_listener_address).run());
}
