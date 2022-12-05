///! Listens for incoming connections and forwards the received data to downstream protocol handler
///!
use prost::Message;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender;
use tokio_stream::StreamExt;
use tokio_util::codec::FramedRead;

use crate::network::codec::ProtoCodec;
use crate::network::listener::NetworkPacket;

pub struct ConnectionHandler {
    stream: TcpStream,
}

impl ConnectionHandler {
    pub fn new(stream: TcpStream) -> ConnectionHandler {
        ConnectionHandler { stream }
    }

    pub async fn run<R: Default + Message>(&mut self, tx_srv: Sender<NetworkPacket<R>>) {
        let peer_addr = self.stream.peer_addr().unwrap().to_string();
        log::debug!("Accepting data from {}", peer_addr);

        let codec = ProtoCodec::<R, R>::new();
        let mut reader = FramedRead::new(&mut self.stream, codec);
        loop {
            if let Some(Ok(req)) = reader.next().await {
                let msg = NetworkPacket {
                    addr: peer_addr.to_owned(),
                    payload: req,
                };
                if let Err(err) = tx_srv.send(msg).await {
                    log::error!("Receiver closed: {}", err);
                    break;
                }
            } else {
                log::debug!("Connection closed by client: {}", peer_addr);
                break;
            }
        }
    }
}
