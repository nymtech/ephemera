//! Network module is responsible for network stack. It passes messages from network to broadcast_protocol.

use async_trait::async_trait;

pub mod client_listener;
pub mod codec;
pub mod ephemera;
pub mod libp2p;
pub mod peer_discovery;

/// Network trait is responsible for network stack. It passes messages from network to broadcast_protocol
/// and from broadcast_protocol to network.
#[async_trait]
pub trait Network {
    async fn run(mut self);
}

pub struct BroadcastMessage<M> {
    pub message: M,
}
