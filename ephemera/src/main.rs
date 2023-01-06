extern crate core;

use futures::executor::block_on;

use crate::app::signatures::broadcast_callback::SigningBroadcastCallBack;
use crate::network::client_listener::NetworkClientListener;
use crate::network::ephemera::EphemeraLauncher;
use crate::settings::Settings;

mod app;
mod broadcast_protocol;
mod cli;
mod crypto;
mod network;
mod settings;

pub mod request {
    include!(concat!(env!("OUT_DIR"), "/broadcast.rs"));
}

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
