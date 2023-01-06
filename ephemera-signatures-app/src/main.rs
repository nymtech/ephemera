mod broadcast_callback;
mod file_backend;

use futures::executor::block_on;
use ephemera::cli;
use ephemera::network::client_listener::NetworkClientListener;
use ephemera::network::ephemera::EphemeraLauncher;
use ephemera::settings::Settings;
use crate::broadcast_callback::SigningBroadcastCallBack;

const CONFIG_DIR: &str = "configuration";

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let args = cli::parse_args();
    let settings = Settings::load(CONFIG_DIR, args.config_file.as_str());

    let app = SigningBroadcastCallBack::new(settings.clone());

    let ephemera = EphemeraLauncher::launch(settings.clone(), app).await;

    //Gets commands from network and sends these to ephemera
    block_on(NetworkClientListener::new(ephemera, settings).run());
}
