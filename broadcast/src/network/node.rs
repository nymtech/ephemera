use prost::Message;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::network::basic::client_handler::ClientRequestHandler;
use crate::network::Network;
use crate::protocols::protocol::Protocol;
use crate::protocols::protocol_handler::ProtocolHandler;
use crate::settings::Settings;

pub struct NodeLauncher;

///! 1) Starts ClientRequestHandler to listen requests from clients
///! 2) Starts Network to listen network protocol messages
///! 3) Starts ProtocolHandler to handle protocol messages
impl NodeLauncher {
    pub fn launch<R, N, P>(
        settings: Settings,
        network: N,
        protocol_handler: ProtocolHandler<R, P>,
    ) -> JoinHandle<()>
    where
        R: Message + Default + Send + 'static,
        N: Network + Send + 'static,
        P: Protocol<R> + Send + 'static,
    {
        let (broadcast_sender, broadcast_receiver) = mpsc::channel(500);
        let (to_protocol, from_protocol) = mpsc::channel(500);

        let client_handler = ClientRequestHandler::new(to_protocol.clone(), &settings);
        tokio::spawn(async move {
            client_handler.run().await;
        });

        tokio::spawn(async move {
            network
                .start(settings, to_protocol.clone(), broadcast_receiver)
                .await;
        });

        tokio::spawn(async move { protocol_handler.run(from_protocol, broadcast_sender).await })
    }
}
