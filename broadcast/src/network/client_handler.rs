use prost::Message;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::Sender;
use tokio_stream::StreamExt;
use tokio_util::codec::FramedRead;

use crate::network::codec::ProtoCodec;
use crate::network::NetworkPacket;
use crate::settings::Settings;

pub struct ClientRequestHandler<M> {
    pub address: String,
    pub command_sender: Sender<NetworkPacket<M>>,
}

impl<M: Message + Default + Send + 'static> ClientRequestHandler<M> {
    pub fn new(command_sender: Sender<NetworkPacket<M>>, settings: &Settings) -> ClientRequestHandler<M> {
        ClientRequestHandler {
            address: settings.client_listener.clone(),
            command_sender,
        }
    }

    pub async fn run(self) {
        let listener = TcpListener::bind(&self.address).await.unwrap();
        log::info!("Listening client commands at: {}", &self.address);
        loop {
            let (socket, _) = listener.accept().await.unwrap();
            let tx = self.command_sender.clone();
            tokio::spawn(async move {
                Self::process(tx, socket).await;
            });
        }
    }

    async fn process(command_sender: Sender<NetworkPacket<M>>, mut stream: TcpStream) {
        let peer_addr = stream.peer_addr().unwrap().to_string();
        let (rd, _wr) = stream.split();
        let codec = ProtoCodec::<M, M>::new();
        let mut reader = FramedRead::new(rd, codec);
        loop {
            if let Some(Ok(cmd)) = reader.next().await {
                let msg = NetworkPacket {
                    addr: peer_addr.clone(),
                    payload: cmd,
                };
                if let Err(err) = command_sender.send(msg).await {
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
