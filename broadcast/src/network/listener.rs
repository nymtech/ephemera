use prost::Message;
use tokio::net::TcpListener;
use tokio::sync::mpsc::Sender;

use crate::network::connection::ConnectionHandler;
use crate::settings::Settings;

#[derive(Debug)]
pub struct NetworkPacket<T> {
    pub addr: String,
    pub payload: T,
}

pub struct NetworkListener {
    settings: Settings,
}

impl NetworkListener {
    pub fn new(settings: Settings) -> NetworkListener {
        Self { settings }
    }

    pub async fn listen<T: Message + Default + 'static>(self, tx_srv: Sender<NetworkPacket<T>>) {
        let listener = TcpListener::bind(self.settings.address.clone())
            .await
            .expect("Failed to start listener");
        log::info!("Accepting connection at {}", self.settings.address.clone());

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    log::trace!("Accepted connection from {}", addr);

                    let tx = tx_srv.clone();
                    tokio::spawn(async move {
                        let mut handler = ConnectionHandler::new(stream);
                        handler.run(tx).await;
                    });
                }
                Err(e) => {
                    log::error!("Failed to accept connection: {}", e);
                }
            }
        }
    }
}
