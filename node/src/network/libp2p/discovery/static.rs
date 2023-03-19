use crate::config::Libp2pConfig;
use crate::network::libp2p::discovery::Peer;
use libp2p::swarm::{
    dummy, ConnectionId, FromSwarm, NetworkBehaviour, NetworkBehaviourAction, PollParameters,
    THandlerInEvent, THandlerOutEvent,
};
use libp2p::Multiaddr;
use libp2p_identity::PeerId;
use std::collections::HashMap;
use std::task::{Context, Poll};

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
    type ConnectionHandler = dummy::ConnectionHandler;
    type OutEvent = ();

    fn new_handler(&mut self) -> Self::ConnectionHandler {
        dummy::ConnectionHandler
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
