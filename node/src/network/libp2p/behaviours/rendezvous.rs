use std::collections::HashMap;
use std::task::{Context, Poll};

use libp2p::swarm::{
    dummy, ConnectionId, FromSwarm, NetworkBehaviour, NetworkBehaviourAction, PollParameters,
    THandlerInEvent, THandlerOutEvent,
};
use libp2p::Multiaddr;
use tokio::task::JoinHandle;

use crate::network::discovery::{PeerDiscovery, PeerInfo};
use crate::network::peer::{Peer, PeerId};

pub(crate) struct RendezvousBehaviour<P: PeerDiscovery> {
    peers: HashMap<PeerId, Peer>,
    previous_peers: HashMap<PeerId, Peer>,
    peer_discovery: Option<P>,
    discovery_channel_tx: tokio::sync::mpsc::UnboundedSender<Vec<PeerInfo>>,
    discovery_channel_rcv: tokio::sync::mpsc::UnboundedReceiver<Vec<PeerInfo>>,
}

impl<P: PeerDiscovery + 'static> RendezvousBehaviour<P> {
    pub fn new(peer_discovery: P) -> Self {
        let (tx, rcv) = tokio::sync::mpsc::unbounded_channel();
        RendezvousBehaviour {
            peers: Default::default(),
            previous_peers: Default::default(),
            peer_discovery: Some(peer_discovery),
            discovery_channel_tx: tx,
            discovery_channel_rcv: rcv,
        }
    }

    pub fn peer_ids(&self) -> Vec<PeerId> {
        self.peers.keys().cloned().collect()
    }

    pub fn peer_ids_ref(&self) -> Vec<&PeerId> {
        self.peers.keys().collect()
    }

    pub fn peers(&self) -> Vec<Peer> {
        self.peers.values().cloned().collect()
    }

    pub fn previous_peer_ids(&self) -> Vec<PeerId> {
        self.previous_peers.keys().cloned().collect()
    }

    pub(crate) async fn spawn(&mut self) -> anyhow::Result<JoinHandle<()>> {
        let tx = self.discovery_channel_tx.clone();
        let mut peer_discovery = self
            .peer_discovery
            .take()
            .ok_or(anyhow::anyhow!("Peer discovery already spawned"))?;

        peer_discovery.poll(tx.clone()).await?;

        let join_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(peer_discovery.get_poll_interval());
            loop {
                interval.tick().await;
                log::trace!("Polling peer discovery (after tick)");
                if let Err(err) = peer_discovery.poll(tx.clone()).await {
                    log::error!("Error while polling peer discovery: {}", err);
                }
            }
        });
        Ok(join_handle)
    }
}

pub(crate) enum Event {
    PeersUpdated,
}

impl<P: PeerDiscovery + 'static> NetworkBehaviour for RendezvousBehaviour<P> {
    type ConnectionHandler = dummy::ConnectionHandler;
    type OutEvent = Event;

    fn new_handler(&mut self) -> Self::ConnectionHandler {
        dummy::ConnectionHandler
    }

    fn addresses_of_peer(&mut self, peer_id: &libp2p_identity::PeerId) -> Vec<Multiaddr> {
        self.peers
            .get(&PeerId(*peer_id))
            .map(|peer| vec![peer.address.clone()])
            .unwrap_or_default()
    }

    fn on_swarm_event(&mut self, _event: FromSwarm<Self::ConnectionHandler>) {
        log::trace!("HttpPeerDiscoveryBehaviour: on_swarm_event");
    }

    fn on_connection_handler_event(
        &mut self,
        _peer_id: libp2p_identity::PeerId,
        _connection_id: ConnectionId,
        _event: THandlerOutEvent<Self>,
    ) {
        log::trace!("HttpPeerDiscoveryBehaviour: on_connection_handler_event");
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
        _params: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<Self::OutEvent, THandlerInEvent<Self>>> {
        if let Poll::Ready(Some(peers)) = self.discovery_channel_rcv.poll_recv(cx) {
            let previous_peers = std::mem::take(&mut self.peers);
            self.previous_peers = previous_peers;

            for peer_info in peers {
                log::info!("Discovered peer {}", peer_info);
                match <PeerInfo as TryInto<Peer>>::try_into(peer_info) {
                    Ok(peer) => {
                        self.peers.insert(peer.peer_id, peer);
                    }
                    Err(err) => {
                        log::error!("Error while converting peer info to peer: {}", err);
                    }
                }
            }
            return Poll::Ready(NetworkBehaviourAction::GenerateEvent(Event::PeersUpdated));
        }
        Poll::Pending
    }
}
