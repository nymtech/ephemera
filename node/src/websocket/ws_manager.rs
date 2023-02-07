use anyhow::Result;
use futures_util::SinkExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use crate::api::types::ApiBlock;
use crate::block::Block;
use crate::config::configuration::WsConfig;

pub struct WsConnection {
    socket: WebSocketStream<TcpStream>,
    pending_messages_rx: broadcast::Receiver<Message>,
}

impl WsConnection {
    pub fn new(
        socket: WebSocketStream<TcpStream>,
        pending_messages_rx: broadcast::Receiver<Message>,
    ) -> WsConnection {
        WsConnection {
            socket,
            pending_messages_rx,
        }
    }

    pub async fn accept_messages(mut self) {
        loop {
            let msg = self.pending_messages_rx.recv().await;
            log::trace!("Received message from broadcast channel: {:?}", msg);
            match msg {
                Ok(msg) => {
                    if let Err(err) = self.socket.send(msg).await {
                        log::error!("Error sending message to websocket client: {:?}", err);
                        break;
                    }
                }
                Err(e) => {
                    log::error!("Error receiving message from broadcast channel: {:?}", e);
                }
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct WsMessageBroadcast {
    pub(crate) pending_messages_tx: broadcast::Sender<Message>,
}

impl WsMessageBroadcast {
    pub(crate) fn new(pending_messages_tx: broadcast::Sender<Message>) -> WsMessageBroadcast {
        WsMessageBroadcast {
            pending_messages_tx,
        }
    }

    pub(crate) fn send_block(&self, block: &Block) -> Result<()> {
        let json = serde_json::to_string::<ApiBlock>(block.into())?;
        let msg = Message::Text(json);
        log::debug!("Sending block to websocket clients: {}", block);
        self.pending_messages_tx.send(msg)?;
        Ok(())
    }
}

pub(crate) struct WsManager {
    pub(crate) config: WsConfig,
    pub(crate) pending_messages_tx: broadcast::Sender<Message>,
    pub(crate) pending_messages_rcv: broadcast::Receiver<Message>,
}

impl WsManager {
    pub(crate) fn new(config: WsConfig) -> Result<(WsManager, WsMessageBroadcast)> {
        let (pending_messages_tx, pending_messages_rcv) = broadcast::channel(1000);
        let ws_message_broadcast = WsMessageBroadcast::new(pending_messages_tx.clone());
        let manager = WsManager {
            config,
            pending_messages_tx,
            pending_messages_rcv,
        };
        Ok((manager, ws_message_broadcast))
    }

    pub async fn bind(mut self) -> Result<()> {
        let listener = TcpListener::bind(&self.config.ws_address).await?;
        log::info!(
            "Listening for websocket connections on {}",
            self.config.ws_address
        );

        loop {
            tokio::select! {
                res = listener.accept() => {
                    match res {
                        Ok((stream, addr)) => {
                            log::debug!("Accepted websocket connection from: {}", addr);
                            self.handle_connection(stream,);
                        }
                        Err(err) => {
                            log::error!("Error accepting websocket connection: {:?}", err);
                        }
                    }
                }
                msg = self.pending_messages_rcv.recv() => {
                        log::trace!("Received message from broadcast channel {msg:?}");
                }
            }
        }
    }

    pub fn handle_connection(&self, stream: TcpStream) {
        let pending_messages_rx = self.pending_messages_tx.subscribe();
        tokio::spawn(async move {
            match tokio_tungstenite::accept_async(stream).await {
                Ok(ws_stream) => {
                    let connection = WsConnection::new(ws_stream, pending_messages_rx);
                    connection.accept_messages().await;
                }
                Err(err) => {
                    log::error!("Error accepting websocket connection: {:?}", err);
                }
            }
            log::debug!("Websocket connection closed");
        });
    }
}
