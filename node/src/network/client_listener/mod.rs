use futures_util::StreamExt;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_util::codec;

use crate::broadcast_protocol::EphemeraSigningRequest;
use crate::network::codec::ProtoCodec;
use crate::network::ephemera::Ephemera;
use crate::request::RbMsg;

pub struct EphemeraNetworkCmdListener {
    pub address: String,
    pub ephemera: Arc<Mutex<Ephemera>>,
}

impl EphemeraNetworkCmdListener {
    pub fn new(ephemera: Ephemera, address: String) -> EphemeraNetworkCmdListener {
        EphemeraNetworkCmdListener {
            address,
            ephemera: Arc::new(Mutex::new(ephemera)),
        }
    }

    pub async fn run(self) {
        match TcpListener::bind(&self.address).await {
            Ok(listener) => {
                log::info!("Listening client commands at: {}", &self.address);
                loop {
                    if let Ok((mut stream, _)) = listener.accept().await {
                        let ephemera = self.ephemera.clone();

                        tokio::spawn(async move {
                            let peer_addr = stream.peer_addr().unwrap().to_string();
                            let (rd, _wr) = stream.split();
                            let codec = ProtoCodec::<RbMsg, RbMsg>::new();
                            let mut reader = codec::FramedRead::new(rd, codec);

                            loop {
                                match reader.next().await {
                                    Some(Ok(rb_msg)) => {
                                        log::debug!("Received msg: {:?}", rb_msg);

                                        ephemera
                                            .lock()
                                            .await
                                            .send_api()
                                            .send_message(EphemeraSigningRequest::new(
                                                peer_addr.clone(),
                                                rb_msg,
                                            ))
                                            .await;
                                    }
                                    Some(Err(err)) => {
                                        log::error!("Error reading msg: {}", err);
                                    }
                                    None => {
                                        log::info!("Connection closed");
                                        break;
                                    }
                                }
                            }
                        });
                    }
                }
            }
            Err(err) => {
                panic!("Error listening client commands: {}", err);
            }
        }
    }
}
