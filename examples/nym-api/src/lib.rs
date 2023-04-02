extern crate core;

use clap::Parser;

pub mod contract;
pub mod epoch;
pub mod metrics;
pub mod nym_api_ephemera;
pub mod nym_api_standalone;
pub mod peers;
pub mod reward;
pub mod storage;

pub const NR_OF_MIX_NODES: usize = 5;
pub const HTTP_NYM_API_HEADER: &str = "X-NYM-API-ID";

#[derive(Parser, Debug, Clone)]
pub struct Args {
    #[clap(long)]
    pub metrics_db_path: String,
    #[clap(long, default_value = "5")]
    pub metrics_collector_interval_seconds: i64,
    #[clap(long, default_value = "20")]
    pub epoch_duration_seconds: u64,
    #[clap(long)]
    pub smart_contract_url: String,
    #[clap(long)]
    pub ephemera_config: String,
    #[clap(long)]
    pub nym_api_id: String,
    #[clap(long, default_value = "1")]
    pub block_polling_interval_seconds: u64,
    #[clap(long, default_value = "60")]
    pub block_polling_max_attempts: u64,
}

#[derive(Parser, Debug, Clone)]
pub struct ContractArgs {
    #[clap(long)]
    pub db_path: String,
    #[clap(long, default_value = "60")]
    pub epoch_duration_seconds: u64,
    #[clap(long)]
    pub url: String,
    #[clap(long)]
    pub ephemera_config: String,
}

pub fn init_logging() {
    if let Ok(directives) = ::std::env::var("RUST_LOG") {
        println!("Logging enabled with directives: {directives}",);
        pretty_env_logger::formatted_timed_builder()
            .parse_filters(&directives)
            .format_timestamp_millis()
            .init();
    } else {
        println!("Logging disabled");
    }
}