///!Starts network listener, protocol handler and broadcaster
///!
///
use std::future::Future;
use std::pin::Pin;

use prost::Message;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::network::broadcaster;
use crate::network::listener::{NetworkListener, NetworkPacket};
use crate::settings::Settings;

pub struct NodeLauncher {
    settings: Settings,
}

impl NodeLauncher {
    pub fn with_settings(settings: Settings) -> NodeLauncher {
        NodeLauncher { settings }
    }

    pub async fn launch<F, R>(self, start_protocol: F) -> JoinHandle<()>
    where
        F: FnOnce(
                mpsc::Receiver<NetworkPacket<R>>,
                mpsc::Sender<broadcaster::BroadcastMessage>,
                Settings,
            ) -> Pin<Box<dyn Future<Output = ()> + Send>>
            + Send
            + 'static,
        R: Message + Default + Send + 'static,
    {
        let (tx_pr, rcv_pr) = mpsc::channel(1024);
        let (tx_br, rcv_br) = mpsc::channel(1024);

        let settings = self.settings.clone();

        //Handle incoming connections
        let network_handle = tokio::spawn(async move {
            let network = NetworkListener::new(settings);
            network.listen(tx_pr).await;
        });

        //Handle broadcast requests
        tokio::spawn(async move {
            let broadcaster = broadcaster::Broadcaster::default();
            broadcaster.run(rcv_br).await;
        });

        //Handle protocol requests
        tokio::spawn(async move {
            start_protocol(rcv_pr, tx_br, self.settings).await;
        });

        network_handle
    }
}
