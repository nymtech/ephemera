use std::fmt::Display;
use std::net::IpAddr;
use std::str::FromStr;

use ::libp2p::multiaddr::Protocol;
use ::libp2p::Multiaddr;
use async_trait::async_trait;
use libp2p_identity::PeerId as Libp2pPeerId;
use serde::{Deserialize, Serialize};

use crate::utilities::{from_base58, Ed25519PublicKey, PublicKey};

pub(crate) mod libp2p;

pub(crate) type PeerIdType = Libp2pPeerId;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId(pub(crate) PeerIdType);

impl PeerId {
    pub fn random() -> Self {
        Self(PeerIdType::random())
    }
    pub fn inner(&self) -> &PeerIdType {
        &self.0
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PeerInfo {
    pub name: String,
    pub address: String,
    pub pub_key: String,
}

#[derive(Debug, Clone)]
pub struct Peer {
    pub name: String,
    pub address: Multiaddr,
    pub public_key: Ed25519PublicKey,
    pub peer_id: PeerId,
}

impl Peer {
    pub fn new(
        name: String,
        address: Multiaddr,
        public_key: Ed25519PublicKey,
        peer_id: PeerId,
    ) -> Self {
        Self {
            name,
            address,
            public_key,
            peer_id,
        }
    }
}

impl TryFrom<PeerInfo> for Peer {
    type Error = anyhow::Error;

    fn try_from(value: PeerInfo) -> Result<Self, Self::Error> {
        let multiaddr = Multiaddr::from_str(value.address.as_str())?;
        let result = from_base58(value.pub_key.clone())?;
        let public_key = Ed25519PublicKey::from_raw_vec(result)?;
        let peer_id = libp2p_identity::PeerId::from_public_key(&public_key.0);
        Ok(Self {
            name: value.name,
            address: multiaddr,
            public_key,
            peer_id: PeerId(peer_id),
        })
    }
}

#[async_trait]
pub trait PeerDiscovery: Send + Sync {
    async fn poll(
        &mut self,
        discovery_channel: tokio::sync::mpsc::UnboundedSender<Vec<PeerInfo>>,
    ) -> anyhow::Result<()>;
    fn get_request_interval_in_sec(&self) -> u64;
}

pub struct Address(pub Multiaddr);

impl TryFrom<Address> for (IpAddr, u16) {
    type Error = std::io::Error;

    fn try_from(addr: Address) -> Result<Self, Self::Error> {
        let mut multiaddr = addr.0;
        if let Some(Protocol::Tcp(port)) = multiaddr.pop() {
            if let Some(Protocol::Ip4(ip)) = multiaddr.pop() {
                return Ok((IpAddr::V4(ip), port));
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "invalid address",
        ))
    }
}
