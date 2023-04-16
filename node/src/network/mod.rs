use std::net::IpAddr;
use std::str::FromStr;

use ::libp2p::{multiaddr::Protocol, Multiaddr};
use log::info;
use thiserror::Error;

pub(crate) mod discovery;
pub(crate) mod libp2p;
pub(crate) mod membership;
pub(crate) mod peer;

#[derive(Error, Debug)]
pub enum AddressError {
    #[error("Failed to parse address: {0}")]
    ParsingError(String),
}

/// A wrapper around a [`Multiaddr`].
/// See [libp2p/multiaddress](https://github.com/libp2p/specs/blob/master/addressing/README.md)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Address(pub Multiaddr);

impl Address {
    pub fn inner(&self) -> Multiaddr {
        self.0.clone()
    }
}

impl From<Multiaddr> for Address {
    fn from(multiaddr: Multiaddr) -> Self {
        Self(multiaddr)
    }
}

/// Supported formats:
/// 1. `<IP>:<PORT>`
/// 2. `/ip4/<IP>/tcp/<PORT>` - this is the format used by libp2p multiaddr
impl FromStr for Address {
    type Err = AddressError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let address: Option<Multiaddr> = match Multiaddr::from_str(s) {
            Ok(multiaddr) => Some(multiaddr),
            Err(err) => {
                info!("Failed to parse multiaddr: {}", err);
                None
            }
        };

        let multi_address = address.or_else(|| match std::net::SocketAddr::from_str(s) {
            Ok(sa) => {
                let mut multiaddr = Multiaddr::empty();
                match sa {
                    std::net::SocketAddr::V4(v4) => {
                        multiaddr.push(Protocol::Ip4(*v4.ip()));
                        multiaddr.push(Protocol::Tcp(v4.port()));
                    }
                    std::net::SocketAddr::V6(v6) => {
                        multiaddr.push(Protocol::Ip6(*v6.ip()));
                        multiaddr.push(Protocol::Tcp(v6.port()));
                    }
                }

                Some(multiaddr)
            }
            Err(err) => {
                info!("Failed to parse socket addr: {err}");
                None
            }
        });

        match multi_address {
            Some(multi_address) => Ok(Self(multi_address)),
            None => Err(AddressError::ParsingError(s.to_string())),
        }
    }
}

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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_multiaddr() {
        "/ip4/127.0.0.1/tcp/1234".parse::<Address>().unwrap();
    }

    #[test]
    fn test_parse_ip_port() {
        "127.0.0.1:1234".parse::<Address>().unwrap();
    }

    #[test]
    fn test_fail_parse_multiaddr_without_port() {
        let result = "/ip4/127.0.0.1/tcp/".parse::<Address>();
        assert!(matches!(result, Err(AddressError::ParsingError(_))));
    }

    #[test]
    fn test_fail_parse_multiaddr_without_ip() {
        let result = "/ip4//tcp/1234".parse::<Address>();
        assert!(matches!(result, Err(AddressError::ParsingError(_))));
    }

    #[test]
    fn test_fail_parse_ip_port_without_port() {
        let result = "127.0.0.1".parse::<Address>();
        assert!(matches!(result, Err(AddressError::ParsingError(_))));
    }

    #[test]
    fn test_fail_parse_ip_port_without_ip() {
        let result = "1234".parse::<Address>();
        assert!(matches!(result, Err(AddressError::ParsingError(_))));
    }
}
