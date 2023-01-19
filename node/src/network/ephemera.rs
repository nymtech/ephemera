use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::broadcast_protocol::protocol_handler::ProtocolHandler;
use crate::broadcast_protocol::signing::signer;
use crate::broadcast_protocol::ProtocolRequest;
use crate::config::configuration::Configuration;
use crate::crypto::ed25519::Ed25519KeyPair;
use crate::crypto::KeyPair;
use crate::http;
use crate::network::libp2p::swarm;
use crate::network::Network;
use rand::Rng;
use crate::api::queries::MessagesApi;

pub struct Ephemera {
    to_protocol: mpsc::Sender<ProtocolRequest>,
    api: MessagesApi,
}

impl Ephemera {
    pub async fn send_message(&mut self, msg: ProtocolRequest) {
        if let Err(err) = self.to_protocol.send(msg).await {
            panic!("Receiver closed: {}, unable to continue", err);
        }
    }

    pub fn api(&self) -> &MessagesApi {
        &self.api
    }
}

pub struct EphemeraLauncher;

impl EphemeraLauncher {
    pub async fn launch(conf: Configuration) -> (Ephemera, JoinHandle<()>) {
        let (to_network, from_protocol) = mpsc::channel(1000);
        let (to_protocol, from_network) = mpsc::channel(1000);

        let network = swarm::SwarmNetwork::new(conf.clone(), to_protocol.clone(), from_protocol);
        tokio::spawn(async move {
            network.run().await;
        });

        //TODO
        let keypair = Ed25519KeyPair::generate().unwrap();
        let signer = signer::Signer::start(keypair, conf.db_config.clone(), conf.ws_config.clone()).await.unwrap();

        let handler_conf = conf.clone();
        let join_handle = tokio::spawn(async move {
            let protocol_handler = ProtocolHandler::new(handler_conf, signer);
            protocol_handler.run(from_network, to_network).await;
        });

        // Obviously we don't want to start the HTTP server on a random port,
        // but this lets multiple nodes start up without port conflicts (most of the time).
        // TODO: we need to get this into the config file, probably needs a bit of discussion.
        let mut rng = rand::thread_rng();
        let random_port: u16 = rng.gen();

        // TODO: this blocks the main tokio runtime. Spawning it results in a futures `Send` problem.
        // I haven't yet found the magic incantation to make Actix Web work seamlessly with an existing
        // tokio runtime.
        http::start(random_port).await;


        let api = MessagesApi::new(conf.db_config.clone());
        let ephemera = Ephemera { to_protocol, api };

        (ephemera, join_handle)
    }
}
