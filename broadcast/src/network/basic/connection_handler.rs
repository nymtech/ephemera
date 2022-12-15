use prost::Message;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender;
use tokio_stream::StreamExt;
use tokio_util::codec::FramedRead;

use crate::network::basic::listener::NetworkPacket;
use crate::network::codec::ProtoCodec;

pub struct ConnectionHandler {
    stream: TcpStream,
}

impl ConnectionHandler {
    pub fn new(stream: TcpStream) -> ConnectionHandler {
        ConnectionHandler { stream }
    }

    pub async fn run<R: Default + Message>(&mut self, tx_srv: Sender<NetworkPacket<R>>) {
        let addr = match self.stream.peer_addr() {
            Ok(address) => address,
            Err(err) => {
                log::error!("failed to get peer address: {}", err);
                return;
            }
        };
        let peer_addr = addr.to_string();
        log::debug!("Accepting data from {}", peer_addr);

        let codec = ProtoCodec::<R, R>::new();
        let mut reader = FramedRead::new(&mut self.stream, codec);
        loop {
            match reader.next().await {
                Some(Ok(data)) => {
                    let packet = NetworkPacket::new(peer_addr.clone(), data);
                    tx_srv.send(packet).await.unwrap();
                }
                Some(Err(err)) => {
                    log::error!("Failed to read data from {}: {}", peer_addr, err);
                    break;
                }
                None => {
                    log::debug!("Connection closed by {}", peer_addr);
                    break;
                }
            }
        }
    }
}
