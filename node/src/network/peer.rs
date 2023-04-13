use std::fmt::Display;

use libp2p::{Multiaddr, PeerId as Libp2pPeerId};
use serde::{Deserialize, Serialize};

use crate::crypto::PublicKey;

pub(crate) type PeerIdType = Libp2pPeerId;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId(pub(crate) PeerIdType);

impl PeerId {
    pub fn random() -> Self {
        Self(PeerIdType::random())
    }

    pub(crate) fn inner(&self) -> &PeerIdType {
        &self.0
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        self.0.to_bytes()
    }

    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(Self(PeerIdType::from_bytes(bytes)?))
    }

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

#[derive(Debug, Clone)]
pub struct Peer {
    pub name: String,
    pub address: Multiaddr,
    pub public_key: PublicKey,
    pub peer_id: PeerId,
}
