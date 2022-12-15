use std::net::IpAddr;
use std::str::FromStr;

use bytes::BytesMut;
use libp2p::core::Multiaddr;
use prost::Message;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_util::codec::Encoder;

use crate::network::basic::connection_handler::ConnectionHandler;
use crate::network::codec::ProtoCodec;
use crate::network::peer_discovery::{Address, PeerDiscovery};
use crate::network::{BroadcastMessage, NetworkPacket};
use crate::settings::Settings;


pub struct NetworkListener<P> {
    settings: Settings,
    peer_discovery: P,
}

impl<P: PeerDiscovery> NetworkListener<P> {
    pub fn new(settings: Settings, peer_discovery: P) -> NetworkListener<P> {
        Self {
            settings,
            peer_discovery,
        }
    }

    pub async fn listen<T: Message + Default + 'static, B: Message + Default>(
        self,
        tx_srv: Sender<NetworkPacket<T>>,
        mut br_rcv: Receiver<BroadcastMessage<B>>,
    ) {
        let multi_addr: Address =
            Address(Multiaddr::from_str(self.settings.address.as_str()).expect("Invalid address"));
        let address: (IpAddr, u16) = multi_addr.try_into().unwrap();
        let listener = TcpListener::bind(address)
            .await
            .expect("Failed to start listener");
        log::info!("Accepting connection at {}", self.settings.address.clone());

        let mut codec = ProtoCodec::<B, B>::new();
        loop {
            tokio::select! {
                // Handle connections from network
                ac = listener.accept() => {
                    match ac {
                        Ok((stream, addr)) => {
                            log::debug!("Accepted connection from {}", addr);

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
                // Handle messages from protocol
                msg = br_rcv.recv() => {
                    if let Some(packet) = msg {
                        let mut buf = BytesMut::new();
                        match codec.encode(packet.message, &mut buf){
                            Ok(_) => {
                               let peers = self.peer_discovery.peer_addresses();
                                for peer in peers {
                                    match TcpStream::connect(peer.clone()).await {
                                        Ok(mut stream) => {
                                            log::trace!("Sending message to peer: {}", peer.clone());
                                            if let Err(e) = stream.write_all(&buf).await {
                                                log::error!("Failed to write to peer: {}", e);
                                            }
                                        }
                                        Err(err) => {
                                            log::error!("Error connecting to peer {}: {}", peer.clone(), err);
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                log::error!("Failed to encode broadcast message: {}", err);
                            }
                        }
                    }
                }
            }
        }
    }
}
