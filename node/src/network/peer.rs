use std::fmt::Display;

use libp2p::PeerId as Libp2pPeerId;
use serde::{Deserialize, Serialize};

use crate::crypto::PublicKey;
use crate::network::Address;

pub(crate) type PeerIdType = Libp2pPeerId;

/// Identifier of a peer of the network.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId(pub(crate) PeerIdType);

impl PeerId {
    pub fn random() -> Self {
        Self(PeerIdType::random())
    }

    /// Returns the internal representation of the peer ID.
    pub(crate) fn inner(&self) -> &PeerIdType {
        &self.0
    }

    /// Returns a raw representation of the peer ID.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.0.to_bytes()
    }

    /// Returns a peer ID from a raw representation.
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(Self(PeerIdType::from_bytes(bytes)?))
    }

    /// Builds a `PeerId` from a public key.
    pub fn from_public_key(public_key: &PublicKey) -> Self {
        Self(PeerIdType::from_public_key(public_key.inner()))
    }
}

impl From<PeerId> for libp2p_identity::PeerId {
    fn from(peer_id: PeerId) -> Self {
        peer_id.0
    }
}

impl From<libp2p_identity::PeerId> for PeerId {
    fn from(peer_id: libp2p_identity::PeerId) -> Self {
        Self(peer_id)
    }
}

impl Display for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub trait ToPeerId {
    fn peer_id(&self) -> PeerId;
}

/// A peer of the network.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Peer {
    /// The peer's ID. It identifies the peer uniquely and is derived from its public key.
    ///
    /// # Example
    ///
    /// ```
    /// use ephemera::crypto::{EphemeraKeypair, Keypair, PublicKey};
    /// use ephemera::peer_discovery::{PeerId, ToPeerId};
    ///
    /// let public_key = Keypair::generate(None).public_key();
    ///
    /// let peer_id = PeerId::from_public_key(&public_key);
    ///
    /// assert_eq!(peer_id, public_key.peer_id());
    ///
    /// ```
    pub peer_id: PeerId,
    /// The peer's public key. It matches PeerId.
    pub public_key: PublicKey,
    /// The peer's address.
    pub address: Address,
    /// The peer's name. It can be arbitrary and is just for logging/display purposes.
    pub name: String,
}
