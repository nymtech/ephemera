use std::thread;
use clap::Parser;
use ephemera::broadcast_protocol::ProtocolRequest;
use ephemera::config::configuration::Configuration;
use ephemera::logging::init_logging;
use ephemera::network::ephemera::EphemeraLauncher;

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

    loop {
        let msg = ephemera_client::pre_prepare_msg("client", "Payload".as_bytes().to_vec());
        let request_id = msg.id.clone();
        println!("Sending request {:?}", msg);

        ephemera_1.send_message(ProtocolRequest::new("client".to_string(), msg)).await;

        thread::sleep(std::time::Duration::from_secs(1));

        let message = ephemera_1.api().get_message(request_id.clone()).unwrap();
        println!("Node 1 received message {:?}", message);

        let message = ephemera_2.api().get_message(request_id.clone()).unwrap();
        println!("Node 2 received message {:?}", message);

        let message = ephemera_3.api().get_message(request_id).unwrap();
        println!("Node 3 received message {:?}", message);

        thread::sleep(std::time::Duration::from_secs(5));
    }
}