///! Knows about the topology of the network and potentially provides a way to discover peers
///!
use crate::settings::Settings;

#[derive(Debug, Clone, Default)]
pub struct Peer {
    pub address: String,
}

impl Peer {
    pub fn new(address: String) -> Peer {
        Self { address }
    }
}

impl From<String> for Peer {
    fn from(address: String) -> Self {
        Self { address }
    }
}

impl From<&str> for Peer {
    fn from(address: &str) -> Self {
        Self {
            address: address.to_string(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Topology {
    pub local: Peer,
    pub peers: Vec<Peer>,
}

impl Topology {
    pub fn new(settings: &Settings) -> Topology {
        let settings = settings.clone();
        Self {
            local: settings.address.into(),
            peers: settings
                .peers
                .into_iter()
                .map(String::into)
                .collect::<Vec<Peer>>(),
        }
    }

    pub fn peers_ref(&self) -> &Vec<Peer> {
        self.peers.as_ref()
    }
}

pub trait PeerDiscovery: Send {
    fn peers_ref(&self) -> &Vec<Peer>;
    fn peer_addresses(&self) -> Vec<String>;
}

#[derive(Debug, Clone, Default)]
pub struct StaticPeerDiscovery {
    topology: Topology,
}

impl StaticPeerDiscovery {
    pub fn new(topology: Topology) -> StaticPeerDiscovery {
        StaticPeerDiscovery { topology }
    }
}

impl PeerDiscovery for StaticPeerDiscovery {
    fn peers_ref(&self) -> &Vec<Peer> {
        self.topology.peers_ref()
    }

    fn peer_addresses(&self) -> Vec<String> {
        self.topology
            .peers_ref()
            .iter()
            .map(|peer| peer.address.clone())
            .collect::<Vec<String>>()
    }
}
