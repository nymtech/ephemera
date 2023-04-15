use std::fmt::Display;

use async_trait::async_trait;
use thiserror::Error;

use crate::crypto::PublicKey;
use crate::network::peer::{Peer, PeerId};
use crate::network::Address;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Information about a peer.
pub struct PeerInfo {
    /// The name of the peer. Can be arbitrary.
    pub name: String,
    /// The address of the peer.
    /// Expected formats:
    /// 1. `<IP>:<PORT>`
    /// 2. `/ip4/<IP>/tcp/<PORT>` - this is the format used by libp2p multiaddr
    pub address: String,
    /// The public key of the peer. It uniquely identifies the peer.
    /// Public key is used to derive the peer id.
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
        let address: Address = value.address.parse()?;
        let public_key = value.pub_key;
        Ok(Self {
            name: value.name,
            address,
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
