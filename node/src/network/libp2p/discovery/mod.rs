//! Knows about the topology of the network and potentially provides a way to discover peers
pub(super) mod r#static;

use std::net::IpAddr;
use std::str::FromStr;

use crate::config::PeerSetting;
use libp2p::multiaddr::Protocol;
use libp2p::Multiaddr;
use libp2p_identity::{PeerId, PublicKey};
use tokio::io;

use crate::utilities::encoding::from_base58;

#[derive(Debug, Clone)]
pub struct Peer {
    pub name: String,
    pub address: Multiaddr,
    pub pub_key: String,
}

impl Peer {
    pub fn new(setting: &PeerSetting) -> Self {
        let multiaddr = Multiaddr::from_str(setting.address.as_str()).unwrap();
        Self {
            name: setting.name.clone(),
            address: multiaddr,
            pub_key: setting.pub_key.clone(),
        }
    }
}

impl TryFrom<Peer> for PeerId {
    type Error = anyhow::Error;
    fn try_from(peer: Peer) -> Result<Self, Self::Error> {
        let bytes = from_base58(peer.pub_key)?;
        let pub_key = PublicKey::from_protobuf_encoding(&bytes[..])?;
        Ok(PeerId::from_public_key(&pub_key))
    }
}

pub struct Address(pub Multiaddr);

impl TryFrom<Address> for (IpAddr, u16) {
    type Error = io::Error;

    fn try_from(addr: Address) -> Result<Self, Self::Error> {
        let mut multiaddr = addr.0;
        if let Some(Protocol::Tcp(port)) = multiaddr.pop() {
            if let Some(Protocol::Ip4(ip)) = multiaddr.pop() {
                return Ok((IpAddr::V4(ip), port));
            }
        }
        Err(io::Error::new(io::ErrorKind::Other, "invalid address"))
    }
}
