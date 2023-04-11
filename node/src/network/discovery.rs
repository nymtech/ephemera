use std::fmt::Display;
use std::net::SocketAddr;
use std::str::FromStr;

use async_trait::async_trait;
use libp2p::multiaddr::Protocol;
use libp2p::Multiaddr;
use thiserror::Error;

use crate::crypto::PublicKey;
use crate::network::peer::{Peer, PeerId};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Information about a peer.
pub struct PeerInfo {
    /// The name of the peer. Can be arbitrary.
    pub name: String,
    /// The address of the peer.
    /// Expected formats:
    /// 1. `<IP>:<PORT>`
    /// 2. `/ip4/<IP>/tcp/<PORT>` - this is the format used by libp2p
    pub address: String,
    /// The public key of the peer.
    pub pub_key: PublicKey,
}

impl Display for PeerInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "name {}, address {}, public key {}",
            self.name, self.address, self.pub_key
        )
    }
}

impl TryFrom<PeerInfo> for Peer {
    type Error = anyhow::Error;

    fn try_from(value: PeerInfo) -> std::result::Result<Self, Self::Error> {
        let multi_address: Option<Multiaddr> = match Multiaddr::from_str(value.address.as_str()) {
            Ok(multiaddr) => Some(multiaddr),
            Err(err) => {
                log::info!("Failed to parse multiaddr: {}", err);
                None
            }
        };

        let multi_address = multi_address.or_else(|| {
            match std::net::SocketAddr::from_str(value.address.as_str()) {
                Ok(sa) => {
                    let mut multiaddr = Multiaddr::empty();
                    match sa {
                        SocketAddr::V4(v4) => {
                            multiaddr.push(Protocol::Ip4(*v4.ip()));
                            multiaddr.push(Protocol::Tcp(v4.port()));
                        }
                        SocketAddr::V6(v6) => {
                            multiaddr.push(Protocol::Ip6(*v6.ip()));
                            multiaddr.push(Protocol::Tcp(v6.port()));
                        }
                    }

                    Some(multiaddr)
                }
                Err(err) => {
                    log::info!("Failed to parse socket addr: {err}");
                    None
                }
            }
        });

        if multi_address.is_none() {
            return Err(anyhow::anyhow!("Failed to parse address"));
        }

        let public_key = value.pub_key;
        Ok(Self {
            name: value.name,
            address: multi_address.unwrap(),
            public_key: public_key.clone(),
            peer_id: PeerId::from_public_key(&public_key),
        })
    }
}

#[derive(Error, Debug)]
pub enum PeerDiscoveryError {
    //Just a placeholder for now
    #[error("PeerDiscoveryError::GeneralError: {0}")]
    GeneralError(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, PeerDiscoveryError>;

/// The PeerDiscovery trait allows the user to implement their own peer discovery mechanism.
#[async_trait]
pub trait PeerDiscovery: Send + Sync {
    /// Ephemera will call this method to poll for new peers.
    /// The implementation should send the new peers to the discovery_channel.
    ///
    /// # Arguments
    /// * `discovery_channel` - The channel to send the new peers to.
    ///
    /// # Returns
    /// * `Result<()>` - An error if the polling failed.
    async fn poll(
        &mut self,
        discovery_channel: tokio::sync::mpsc::UnboundedSender<Vec<PeerInfo>>,
    ) -> Result<()>;

    /// Ephemera will call this method to get the interval between each poll.
    ///
    /// # Returns
    /// * `std::time::Duration` - The interval between each poll.
    fn get_poll_interval(&self) -> std::time::Duration;
}

#[cfg(test)]
mod test {
    use crate::crypto::{EphemeraKeypair, Keypair};

    use super::*;

    #[test]
    fn test_parse_peer_info_multiaddr() {
        let peer_info = PeerInfo {
            name: "test".to_string(),
            address: "/ip4/127.0.0.1/tcp/1234".to_string(),
            pub_key: Keypair::generate(None).public_key(),
        };

        let peer = Peer::try_from(peer_info);
        assert!(peer.is_ok());
    }

    #[test]
    fn test_parse_peer_info_ip_port() {
        let peer_info = PeerInfo {
            name: "test".to_string(),
            address: "127.0.0.1:1234".to_string(),
            pub_key: Keypair::generate(None).public_key(),
        };

        let peer = Peer::try_from(peer_info);
        assert!(peer.is_ok());
    }
}
