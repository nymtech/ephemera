use async_trait::async_trait;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::network::basic::listener::NetworkPacket;
use crate::settings::Settings;

pub mod basic;
pub mod codec;
pub mod libp2p;
pub mod node;
pub mod peer_discovery;

///! Network trait is responsible for network stack. It passes messages from network to protocol
/// and from protocol to network.
#[async_trait]
pub trait Network {
    async fn start<R: prost::Message + Default + 'static, B: prost::Message + Default + 'static>(
        mut self,
        settings: Settings,
        // Channel to send messages to protocol
        network_sender: Sender<NetworkPacket<R>>,
        // Channel to receive messages from protocol
        broadcast_receiver: Receiver<BroadcastMessage<B>>,
    );
}

pub struct BroadcastMessage<M> {
    pub message: M,
}
