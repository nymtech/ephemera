use crate::crypto::PublicKey;
use async_trait::async_trait;
use libp2p::Multiaddr;
use std::str::FromStr;

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
    /// 3. `<DNS>:<PORT>`
    pub address: String,
    /// The public key of the peer.
    pub pub_key: PublicKey,
}

impl TryFrom<PeerInfo> for Peer {
    type Error = anyhow::Error;

    fn try_from(value: PeerInfo) -> Result<Self, Self::Error> {
        let multiaddr = Multiaddr::from_str(value.address.as_str())?;
        let public_key = value.pub_key;
        Ok(Self {
            name: value.name,
            address: multiaddr,
            public_key: public_key.clone(),
            peer_id: PeerId::from_public_key(&public_key),
        })
    }
}

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
    /// * `anyhow::Result<()>` - The result of the operation.
    async fn poll(
        &mut self,
        discovery_channel: tokio::sync::mpsc::UnboundedSender<Vec<PeerInfo>>,
    ) -> anyhow::Result<()>;

    /// Ephemera will call this method to get the interval between each poll.
    ///
    /// # Returns
    /// * `std::time::Duration` - The interval between each poll.
    fn get_poll_interval(&self) -> std::time::Duration;
}
