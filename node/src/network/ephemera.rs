use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::api::queries::MessagesQueryApi;
use crate::api::send::MessageSendApi;
use crate::broadcast_protocol::protocol_handler::ProtocolHandler;
use crate::broadcast_protocol::signing::signer;

use crate::config::configuration::Configuration;
use crate::crypto::ed25519::Ed25519KeyPair;
use crate::crypto::KeyPair;
use crate::http;
use crate::network::libp2p::swarm;
use crate::network::Network;

/// Interacting with Ephemera.
///
/// It is possible to use Ephemera API in two ways - Ephemera Rust instance or over http.
///
/// 1. Rust instance
///     A. Ephemera.send_message() - for sending messages
///     B. Ephemera.api() functions - for querying messages
///
/// 2. Http
///     C. http - for querying messages
///     D. http - for sending messages

pub struct Ephemera {
    query_api: MessagesQueryApi,
    send_api: MessageSendApi,
}

impl Ephemera {
    pub fn new(query_api: MessagesQueryApi, send_api: MessageSendApi) -> Self {
        Self { query_api, send_api }
    }

    pub fn send_api(&mut self) -> &mut MessageSendApi {
        &mut self.send_api
    }

    pub fn query_api(&self) -> &MessagesQueryApi {
        &self.query_api
    }
}

pub struct EphemeraLauncher;

impl EphemeraLauncher {
    pub async fn launch(config: Configuration) -> (Ephemera, JoinHandle<()>) {
        let (to_network, from_protocol) = mpsc::channel(1000);
        let (to_protocol, from_network) = mpsc::channel(1000);

        // Network layer to send messages to other nodes, underlying mechanism is libp2p gossipsub
        let network = swarm::SwarmNetwork::new(config.clone(), to_protocol.clone(), from_protocol);
        tokio::spawn(async move {
            network.run().await;
        });

        // Protocol layer to handle incoming messages, sign them and broadcast signed messages to other nodes
        //TODO
        let keypair = Ed25519KeyPair::generate().unwrap();
        let signer = signer::Signer::start(keypair, config.db_config.clone(), config.ws_config.clone())
            .await
            .unwrap();

        let handler_conf = config.clone();
        let join_handle = tokio::spawn(async move {
            let protocol_handler = ProtocolHandler::new(handler_conf, signer);
            protocol_handler.run(from_network, to_network).await;
        });


        // API layer to submit and query messages
        let send_msg_api = MessageSendApi::new(to_protocol.clone());

        let http_conf = config.clone();
        let http_send_api = send_msg_api.clone();
        tokio::spawn(async move {
            let http_server = http::start(http_conf, http_send_api).unwrap();
            http_server.await.unwrap();
        });

        let ephemera = Ephemera::new(MessagesQueryApi::new(config.db_config), send_msg_api);
        (ephemera, join_handle)
    }
}
