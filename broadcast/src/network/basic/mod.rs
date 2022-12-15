use async_trait::async_trait;
use prost::Message;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::network::basic::listener::{NetworkListener, NetworkPacket};
use crate::network::peer_discovery::StaticPeerDiscovery;
use crate::network::{BroadcastMessage, Network};
use crate::settings::Settings;

pub mod client_handler;
pub mod connection_handler;
pub mod listener;

pub struct BasicNetwork {
    pub peer_discovery: StaticPeerDiscovery,
}

#[async_trait]
impl Network for BasicNetwork {
    async fn start<R: Message + Default + 'static, B: Message + Default + 'static>(
        mut self,
        settings: Settings,
        network_sender: Sender<NetworkPacket<R>>,
        broadcast_receiver: Receiver<BroadcastMessage<B>>,
    ) {
        let tx_pr = network_sender;
        tokio::spawn(async move {
            let listener = NetworkListener::new(settings, self.peer_discovery);
            listener.listen(tx_pr, broadcast_receiver).await;
        });
    }
}

pub fn create_basic_network(settings: &Settings) -> BasicNetwork {
    let peer_discovery = StaticPeerDiscovery::new(settings);
    BasicNetwork { peer_discovery }
}
