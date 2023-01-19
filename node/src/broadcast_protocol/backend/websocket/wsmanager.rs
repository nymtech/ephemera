use std::io;
use tokio::net::{TcpListener, TcpStream};

use tokio_tungstenite::{tungstenite, WebSocketStream};

use anyhow::Result;

use futures_util::{StreamExt, TryStreamExt};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_stream::wrappers;

use tokio_tungstenite::tungstenite::Message;

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

    pub async fn handle_messages(self) {
        wrappers::BroadcastStream::new(self.pending_messages_rx)
            .map_err(|e| tungstenite::Error::from(io::Error::new(io::ErrorKind::Other, e)))
            .forward(self.socket)
            .await
            .unwrap();
    }
}

pub struct WsManagerHandle {
    pending_messages_tx: broadcast::Sender<Message>,
    #[allow(dead_code)]
    pending_messages_rx: broadcast::Receiver<Message>,
}

impl WsManagerHandle {
    pub async fn send(&mut self, msg: Vec<u8>) -> Result<()> {
        let ws_msg = Message::binary(msg);
        self.pending_messages_tx.send(ws_msg)?;
        Ok(())
    }
}

pub struct WsManager {
    pending_messages_tx: broadcast::Sender<Message>,
}

impl WsManager {
    pub fn new(pending_messages_tx: broadcast::Sender<Message>) -> Self {
        WsManager { pending_messages_tx }
    }

    pub async fn start(listen_addr: String) -> Result<(WsManagerHandle, JoinHandle<()>)> {
        let socket = TcpListener::bind(listen_addr.clone()).await.unwrap();
        log::info!("Accepting websocket connections on {}", listen_addr);

        let (pending_messages_tx, _pending_messages_rx) = broadcast::channel(1000);
        let handle = WsManagerHandle {
            pending_messages_tx: pending_messages_tx.clone(),
            pending_messages_rx: pending_messages_tx.subscribe(),
        };

        let join_handle = tokio::spawn(async move {
            let mut ws_manager = WsManager::new(pending_messages_tx.clone());
            loop {
                tokio::select! {
                    maybe_stream = socket.accept() => {
                        match maybe_stream {
                            Ok((stream, addr)) => {
                                log::info!("New websocket connection from {}", addr);
                                let ws_stream = tokio_tungstenite::accept_async(stream).await.unwrap();
                                ws_manager.handle_incoming(ws_stream,).await;
                            },
                            Err(e) => {
                                log::error!("Error accepting websocket connection: {}", e);
                            }
                        }
                    },
                }
            }
        });
        Ok((handle, join_handle))
    }

    pub async fn handle_incoming(&mut self, stream: WebSocketStream<TcpStream>) {
        let pending_messages_rx = self.pending_messages_tx.subscribe();
        tokio::spawn(async move {
            let connection = WsConnection::new(stream, pending_messages_rx);
            connection.handle_messages().await;
        });
    }
}
