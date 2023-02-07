use std::path::PathBuf;

use clap::Parser;

use crate::config::configuration::Configuration;
use crate::ephemera::Ephemera;

#[derive(Debug, Clone, Parser)]
pub struct RunNodeCmd {
    #[clap(short, long)]
    pub config_file: String,
}

impl RunNodeCmd {
    pub async fn execute(&self) {
        let conf = match Configuration::try_load(PathBuf::from(self.config_file.as_str())) {
            Ok(conf) => conf,
            Err(err) => panic!("Error loading configuration file: {err:?}",),
        };

        let mut ephemera = Ephemera::start_services(conf.clone()).await;

        tokio::spawn(async move {
            ephemera.run().await;
        })
        .await
        .unwrap();
    }
}
