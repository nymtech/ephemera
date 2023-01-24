use clap::Parser;
use ephemera::broadcast_protocol::{pre_prepare_msg, EphemeraSigningRequest};
use ephemera::config::configuration::Configuration;
use ephemera::logging::init_logging;
use ephemera::network::ephemera::EphemeraLauncher;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread;
use uuid::Uuid;

#[derive(Parser)]
struct Args {
    #[clap(long)]
    node_config_1: String,
    #[clap(long)]
    node_config_2: String,
    #[clap(long)]
    node_config_3: String,
}

#[tokio::main]
async fn main() {
    init_logging();

    let args = Args::parse();

    let settings_1 = Configuration::try_load(args.node_config_1.into()).unwrap();
    let (mut ephemera_1, _) = EphemeraLauncher::launch(settings_1).await;

    let settings_2 = Configuration::try_load(args.node_config_2.into()).unwrap();
    let (ephemera_2, _) = EphemeraLauncher::launch(settings_2).await;

    let settings_3 = Configuration::try_load(args.node_config_3.into()).unwrap();
    let (ephemera_3, _) = EphemeraLauncher::launch(settings_3).await;

    thread::sleep(std::time::Duration::from_secs(5));

    let stop = Arc::new(AtomicBool::new(false));
    let stop_trigger = stop.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        stop_trigger.store(true, std::sync::atomic::Ordering::Relaxed);
        println!("Stopping...");
    });

    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
        let msg = pre_prepare_msg(
            "client".to_string(),
            Uuid::new_v4().to_string(),
            "Embedded API test".as_bytes().to_vec(),
        );
        let request_id = msg.id.clone();
        println!("Sending request {:?}", msg);

        ephemera_1
            .send_api()
            .send_message(EphemeraSigningRequest::new("client".to_string(), msg))
            .await;

        thread::sleep(std::time::Duration::from_secs(1));

        let message = ephemera_1
            .query_api()
            .get_message_by_request_id(request_id.clone())
            .unwrap();
        println!("Node 1 received message {:?}", message);

        let message = ephemera_2
            .query_api()
            .get_message_by_request_id(request_id.clone())
            .unwrap();
        println!("Node 2 received message {:?}", message);

        let message = ephemera_3
            .query_api()
            .get_message_by_request_id(request_id)
            .unwrap();
        println!("Node 3 received message {:?}", message);

        thread::sleep(std::time::Duration::from_secs(5));
    }
}
