use std::net::IpAddr;

use ::libp2p::multiaddr::Protocol;
use ::libp2p::Multiaddr;

pub mod discovery;
pub(crate) mod libp2p;
pub(crate) mod peer;
pub(crate) mod topology;

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
