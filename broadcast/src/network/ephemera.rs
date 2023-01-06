use prost::Message;
use prost_types::Timestamp;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use crate::app::signatures::file_backend::SignaturesBackend;
use crate::app::signatures::protocol_callback::SigningBroadcastCallBack;
use crate::crypto::ed25519::{Ed25519KeyPair, KeyPair};

use crate::reliable_broadcast::protocol_handler::ProtocolHandler;
use crate::network::client_handler::client_handler::NetworkCommandListener;
use crate::network::Network;
use crate::settings::Settings;
use crate::network::libp2p::swarm;
use crate::reliable_broadcast::protocol::BroadcastProtocol;
use crate::reliable_broadcast::{BroadcastCallBack, ProtocolRequest};
use crate::reliable_broadcast::quorum::BasicQuorum;
use crate::request::rb_msg::ReliableBroadcast::PrePrepare;
use crate::request::{PrePrepareMsg, RbMsg};

#[derive(Clone)]
pub struct Ephemera {
    to_protocol: mpsc::Sender<ProtocolRequest>,
}

impl Ephemera {
    pub async fn send_message(&mut self, msg: ProtocolRequest) {
        if let Err(err) = self.to_protocol.send(msg).await {
            panic!("Receiver closed: {}, unable to continue", err);
        }
    }
}

pub struct EphemeraLauncher;

impl EphemeraLauncher {
    pub async fn launch<C: BroadcastCallBack + 'static>(settings: Settings, protocol_callback: C) -> Ephemera {
        let (to_network, from_protocol) = mpsc::channel(500);
        let (to_protocol, from_network) = mpsc::channel(500);

        let network = swarm::SwarmNetwork::new(settings.clone(), to_protocol.clone(), from_protocol);
        tokio::spawn(async move {
            network.run().await;
        });

        tokio::spawn(async move {
            let protocol_handler = ProtocolHandler::new(settings, protocol_callback);
            protocol_handler.start(from_network, to_network).await;
        });

        Ephemera {
            to_protocol: to_protocol.clone(),
        }
    }
}
