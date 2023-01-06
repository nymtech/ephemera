///! Knows about the topology of the network and potentially provides a way to discover peers
///!
use std::collections::HashMap;
use std::net::IpAddr;
use std::str::FromStr;
use std::task::{Context, Poll};

use libp2p::core::{PeerId, PublicKey};
use libp2p::multiaddr::Protocol;
use libp2p::swarm::{dummy::ConnectionHandler, NetworkBehaviour, NetworkBehaviourAction, PollParameters};
use libp2p::Multiaddr;
use tokio::io;

use crate::settings::{PeerSetting, Settings};

#[derive(Debug, Clone)]
pub struct Peer {
    pub name: String,
    pub address: Multiaddr,
    pub pub_key: String,
}

impl From<&PeerSetting> for Peer {
    fn from(setting: &PeerSetting) -> Self {
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
        let bytes = hex::decode(peer.pub_key)?;
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

#[derive(Debug, Clone)]
pub struct StaticPeerDiscovery {
    peers: HashMap<PeerId, Peer>,
}

impl StaticPeerDiscovery {
    pub fn new(settings: Settings) -> StaticPeerDiscovery {
        let mut peers: HashMap<PeerId, Peer> = HashMap::new();
        for peer in settings.gossipsub.peers.iter() {
            let peer: Peer = Peer::from(peer);
            let peer_id: PeerId = peer.clone().try_into().unwrap();
            peers.insert(peer_id, peer);
        }
        StaticPeerDiscovery { peers }
    }

    pub fn peer_ids(&self) -> Vec<PeerId> {
        self.peers.keys().cloned().collect()
    }
}

//Temporary glue code to tell to libp2p about the peers from our configuration file.
//We register StaticPeerDiscovery with libp2p swarm so when swarm looks for peer addresses then it will
//find these from StaticPeerDiscovery.peers.
impl NetworkBehaviour for StaticPeerDiscovery {
    type ConnectionHandler = ConnectionHandler;
    type OutEvent = ();

    fn new_handler(&mut self) -> Self::ConnectionHandler {
        ConnectionHandler
    }

    fn addresses_of_peer(&mut self, peer_id: &PeerId) -> Vec<Multiaddr> {
        self.peers
            .get(peer_id)
            .map(|peer| vec![peer.address.clone()])
            .unwrap_or_default()
    }

    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
        _params: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<Self::OutEvent, Self::ConnectionHandler>> {
        Poll::Pending
    }
}
