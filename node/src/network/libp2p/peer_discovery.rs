//! Knows about the topology of the network and potentially provides a way to discover peers
//!
use std::collections::HashMap;
use std::net::IpAddr;
use std::str::FromStr;
use std::task::{Context, Poll};

use libp2p::multiaddr::Protocol;
use libp2p::swarm::{
    dummy::ConnectionHandler, ConnectionId, FromSwarm, NetworkBehaviour, NetworkBehaviourAction,
    PollParameters, THandlerInEvent, THandlerOutEvent,
};
use libp2p::Multiaddr;
use libp2p_identity::{PeerId, PublicKey};
use tokio::io;

use crate::config::{Libp2pConfig, PeerSetting};
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

#[derive(Debug, Clone)]
pub struct StaticPeerDiscovery {
    peers: HashMap<PeerId, Peer>,
}

impl StaticPeerDiscovery {
    pub fn new(conf: &Libp2pConfig) -> StaticPeerDiscovery {
        let mut peers: HashMap<PeerId, Peer> = HashMap::new();
        for setting in conf.peers.iter() {
            let peer: Peer = Peer::new(setting);
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

    fn on_swarm_event(&mut self, _event: FromSwarm<Self::ConnectionHandler>) {
        log::debug!("StaticPeerDiscovery: on_swarm_event");
    }

    fn on_connection_handler_event(
        &mut self,
        _peer_id: PeerId,
        _connection_id: ConnectionId,
        _event: THandlerOutEvent<Self>,
    ) {
        log::debug!("StaticPeerDiscovery: on_connection_handler_event");
    }

    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
        _params: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<Self::OutEvent, THandlerInEvent<Self>>> {
        Poll::Pending
    }
}
