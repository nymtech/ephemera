use async_trait::async_trait;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::settings::Settings;

pub mod basic;
pub mod codec;
pub mod libp2p;
pub mod node;
pub mod peer_discovery;
pub mod client_handler;

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

#[derive(Debug)]
pub struct NetworkPacket<R> {
    pub addr: String,
    pub payload: R,
}

impl<R> NetworkPacket<R> {
    pub fn new(addr: String, payload: R) -> NetworkPacket<R> {
        NetworkPacket { addr, payload }
    }
}
