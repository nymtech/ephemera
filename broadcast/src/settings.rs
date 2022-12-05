use std::env;

use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Settings {
    pub address: String,
    pub peers: Vec<String>,
    pub private_key: String,
    pub signatures_file: String,
}

impl Settings {
    pub fn load(config_dir: &str, file: &str) -> Settings {
        let configuration_directory = env::current_dir()
            .expect("Failed to determine the current directory")
            .join(config_dir);
        let config = config::Config::builder()
            .add_source(config::File::from(configuration_directory.join(file)))
            .build()
            .expect("Failed to load configuration");

        config
            .try_deserialize()
            .expect("Failed to deserialize configuration")
    }
}
