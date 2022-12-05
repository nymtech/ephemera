///! Basically just a loop over a list of addresses to open a connection to
///! and then send a message to each of them.
///!
///! Should be part of a more general broadcasting module.

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;

pub struct BroadcastMessage {
    pub peers: Vec<String>,
    pub payload: Vec<u8>,
}

#[derive(Default)]
pub struct Broadcaster {}

impl Broadcaster {
    pub async fn run(self, mut br_rcv: Receiver<BroadcastMessage>) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(packet) = br_rcv.recv().await {
                for peer in packet.peers {
                    if let Ok(mut conn) = TcpStream::connect(peer.clone()).await {
                        log::debug!("Broadcasting to peer: {}", peer.clone());

                        if let Err(err) = conn.write_all(&packet.payload).await {
                            log::error!("Error broadcasting to peer {}: {}", peer.clone(), err);
                        }
                    }
                }
            }
        })
    }
}
