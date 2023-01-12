use tokio::net::TcpListener;
use tokio_stream::StreamExt;
use tokio_util::codec::FramedRead;

use crate::broadcast_protocol::ProtocolRequest;
use crate::network::codec::ProtoCodec;
use crate::network::ephemera::Ephemera;
use crate::request::RbMsg;

pub struct EphemeraNetworkCmdListener {
    pub address: String,
    pub ephemera: Ephemera,
}

impl EphemeraNetworkCmdListener {
    pub fn new(ephemera: Ephemera, address: String) -> EphemeraNetworkCmdListener {
        EphemeraNetworkCmdListener { address, ephemera }
    }

    pub async fn run(mut self) {
        match TcpListener::bind(&self.address).await {
            Ok(listener) => {
                log::info!("Listening client commands at: {}", &self.address);
                loop {
                    if let Ok((mut stream, _)) = listener.accept().await {
                        let peer_addr = stream.peer_addr().unwrap().to_string();
                        let (rd, _wr) = stream.split();
                        let codec = ProtoCodec::<RbMsg, RbMsg>::new();
                        let mut reader = FramedRead::new(rd, codec);
                        loop {
                            if let Some(Ok(cmd)) = reader.next().await {
                                let msg = ProtocolRequest::new(peer_addr.clone(), cmd);
                                self.ephemera.send_message(msg).await;
                            } else {
                                log::debug!("Connection closed by client: {}", peer_addr);
                                break;
                            }
                        }
                    }
                }
            }
            Err(err) => {
                panic!("Error listening client commands: {}", err);
            }
        }
    }
}
