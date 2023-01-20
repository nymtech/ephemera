use crate::config::configuration::Configuration;
use crate::network::client_listener::EphemeraNetworkCmdListener;
use crate::network::ephemera::EphemeraLauncher;
use clap::Parser;
use futures::executor::block_on;
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
pub struct RunNodeCmd {
    #[clap(short, long)]
    pub config_file: String,
}

impl RunNodeCmd {
    pub async fn execute(&self) {
        let conf = match Configuration::try_load(PathBuf::from(self.config_file.as_str())) {
            Ok(conf) => conf,
            Err(err) => panic!("Error loading configuration file: {:?}", err),
        };
        log::info!("Starting ephemera node {}", self.config_file);
        let (ephemera, _) = EphemeraLauncher::launch(conf.clone()).await;

        //Gets commands from network and sends these to ephemera
        block_on(
            EphemeraNetworkCmdListener::new(ephemera, conf.network_client_listener_config.address).run(),
        );
    }
}
