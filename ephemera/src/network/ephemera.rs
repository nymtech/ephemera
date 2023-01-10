use tokio::sync::mpsc;

use crate::broadcast_protocol::protocol_handler::ProtocolHandler;
use crate::broadcast_protocol::{BroadcastCallBack, ProtocolRequest};
use crate::config::configuration::Configuration;
use crate::network::libp2p::swarm;
use crate::network::Network;

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
    pub async fn launch<C: BroadcastCallBack + 'static>(
        conf: Configuration,
        protocol_callback: C,
    ) -> Ephemera {
        let (to_network, from_protocol) = mpsc::channel(500);
        let (to_protocol, from_network) = mpsc::channel(500);

        let network = swarm::SwarmNetwork::new(conf.clone(), to_protocol.clone(), from_protocol);
        tokio::spawn(async move {
            network.run().await;
        });

        tokio::spawn(async move {
            let protocol_handler = ProtocolHandler::new(conf, protocol_callback);
            protocol_handler.run(from_network, to_network).await;
        });

        Ephemera { to_protocol }
    }
}
