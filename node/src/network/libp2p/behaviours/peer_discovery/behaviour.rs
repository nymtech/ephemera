use std::collections::HashMap;
use std::task::{Context, Poll};
use std::time::Duration;

use libp2p::core::ConnectedPoint;
use libp2p::{
    swarm::{
        behaviour::ConnectionEstablished,
        dial_opts::{DialOpts, PeerCondition},
        ConnectionClosed, ConnectionId, DialFailure, FromSwarm, NetworkBehaviour,
        NetworkBehaviourAction, PollParameters, THandlerInEvent, THandlerOutEvent,
    },
    Multiaddr,
};
use tokio::task::JoinHandle;
use tokio::time;

use crate::network::libp2p::behaviours::peer_discovery::{
    calculate_minimum_available_nodes, MAX_DIAL_ATTEMPT_ROUNDS,
};
use crate::network::{
    discovery::{PeerDiscovery, PeerInfo},
    libp2p::behaviours::peer_discovery::handler::Handler,
    peer::{Peer, PeerId},
};

#[derive(Debug, Default)]
struct WaitingDial {
    new_topology: HashMap<PeerId, Peer>,
    connected_peers: HashMap<PeerId, Peer>,
    waiting_to_dial: Vec<Peer>,
    dial_failed: Vec<Peer>,
    dial_attempts: usize,
    interval_between_dial_attempts: Option<time::Interval>,
}

#[derive(Debug, Default)]
struct NotifyPeersUpdated {
    new_topology: HashMap<PeerId, Peer>,
}

enum State {
    WaitingPeers,
    WaitingDial(WaitingDial),
    NotifyPeersUpdated(NotifyPeersUpdated),
}

pub(crate) struct RendezvousBehaviour<P: PeerDiscovery> {
    peers: HashMap<PeerId, Peer>,
    local_peer_id: PeerId,
    previous_peers: HashMap<PeerId, Peer>,
    peer_discovery: Option<P>,
    discovery_channel_tx: tokio::sync::mpsc::UnboundedSender<Vec<PeerInfo>>,
    discovery_channel_rcv: tokio::sync::mpsc::UnboundedReceiver<Vec<PeerInfo>>,
    state: State,
}

impl<P: PeerDiscovery + 'static> RendezvousBehaviour<P> {
    pub(crate) fn new(peer_discovery: P, local_peer_id: PeerId) -> Self {
        let (tx, rcv) = tokio::sync::mpsc::unbounded_channel();
        RendezvousBehaviour {
            peers: Default::default(),
            local_peer_id,
            previous_peers: Default::default(),
            peer_discovery: Some(peer_discovery),
            discovery_channel_tx: tx,
            discovery_channel_rcv: rcv,
            state: State::WaitingPeers,
        }
    }

    /// Returns the list of peers that are part of most recent topology.
    /// Including local peer.
    pub(crate) fn peer_ids(&self) -> Vec<PeerId> {
        self.peers.keys().cloned().collect()
    }

    /// Returns the list of peers that are part of most recent topology.
    /// Including local peer.
    pub(crate) fn peers(&self) -> Vec<Peer> {
        self.peers.values().cloned().collect()
    }

    pub(crate) fn previous_peer_ids(&self) -> Vec<PeerId> {
        self.previous_peers.keys().cloned().collect()
    }

    pub(crate) async fn spawn(&mut self) -> anyhow::Result<JoinHandle<()>> {
        let tx = self.discovery_channel_tx.clone();
        let mut peer_discovery = self
            .peer_discovery
            .take()
            .ok_or(anyhow::anyhow!("Peer discovery already spawned"))?;

        let join_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(peer_discovery.get_poll_interval());
            loop {
                interval.tick().await;
                log::debug!("Polling peer discovery (after tick)");
                if let Err(err) = peer_discovery.poll(tx.clone()).await {
                    log::error!("Error while polling peer discovery: {}", err);
                }
            }
        });
        Ok(join_handle)
    }
}

pub(crate) enum Event {
    PeerUpdatePending,
    PeersUpdated,
    LocalRemoved,
}

impl<P: PeerDiscovery + 'static> NetworkBehaviour for RendezvousBehaviour<P> {
    type ConnectionHandler = Handler;
    type OutEvent = Event;

    fn new_handler(&mut self) -> Self::ConnectionHandler {
        Handler
    }

    fn addresses_of_peer(&mut self, peer_id: &libp2p_identity::PeerId) -> Vec<Multiaddr> {
        self.peers
            .get(&PeerId(*peer_id))
            .map(|peer| vec![peer.address.clone()])
            .unwrap_or_default()
    }

    fn on_swarm_event(&mut self, event: FromSwarm<Self::ConnectionHandler>) {
        match event {
            FromSwarm::ConnectionEstablished(ConnectionEstablished {
                peer_id,
                connection_id: _,
                endpoint: ConnectedPoint::Dialer { .. },
                failed_addresses: _,
                other_established: _,
            }) => {
                if let State::WaitingDial(WaitingDial {
                    new_topology,
                    connected_peers,
                    waiting_to_dial: _,
                    dial_failed,
                    dial_attempts,
                    interval_between_dial_attempts: _,
                }) = &mut self.state
                {
                    let ephemera_id = peer_id.into();
                    if let Some(peer) = new_topology.get(&ephemera_id) {
                        connected_peers.insert(ephemera_id, peer.clone());
                        dial_failed.retain(|peer| peer.peer_id != ephemera_id);
                    }

                    if connected_peers.len() == new_topology.len()
                        || *dial_attempts >= MAX_DIAL_ATTEMPT_ROUNDS
                    {
                        let minimum_available_nodes =
                            calculate_minimum_available_nodes(new_topology.len());
                        if connected_peers.len() >= minimum_available_nodes {
                            let notify_peers_updated = NotifyPeersUpdated {
                                new_topology: connected_peers.clone(),
                            };
                            self.state = State::NotifyPeersUpdated(notify_peers_updated);
                        }
                    }
                }
            }

            FromSwarm::ConnectionClosed(ConnectionClosed { .. }) => {}
            FromSwarm::DialFailure(DialFailure {
                peer_id: Some(peer_id),
                error,
                connection_id: _,
            }) => {
                if let State::WaitingDial(WaitingDial {
                    new_topology,
                    connected_peers: _,
                    waiting_to_dial: _,
                    dial_failed,
                    dial_attempts: _,
                    interval_between_dial_attempts: _,
                }) = &mut self.state
                {
                    log::trace!("DialFailure, peer_id: {peer_id:?}, error: {error:?}",);

                    let ephemera_id = peer_id.into();
                    if let Some(peer) = new_topology.get(&ephemera_id) {
                        dial_failed.push(peer.clone());
                    }
                }
            }
            _ => {}
        }
    }

    fn on_connection_handler_event(
        &mut self,
        _peer_id: libp2p_identity::PeerId,
        _connection_id: ConnectionId,
        _event: THandlerOutEvent<Self>,
    ) {
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
        _params: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<Self::OutEvent, THandlerInEvent<Self>>> {
        match &mut self.state {
            State::WaitingPeers => {
                if let Poll::Ready(Some(peers)) = self.discovery_channel_rcv.poll_recv(cx) {
                    log::info!("Received peers from discovery: {:?}", peers);

                    self.previous_peers = std::mem::take(&mut self.peers);

                    let mut waiting_dial = WaitingDial::default();

                    for peer_info in peers {
                        match <PeerInfo as TryInto<Peer>>::try_into(peer_info) {
                            Ok(peer) => {
                                waiting_dial.new_topology.insert(peer.peer_id, peer.clone());
                            }
                            Err(err) => {
                                log::error!("Error while converting peer info to peer: {}", err);
                            }
                        }
                    }

                    match waiting_dial.new_topology.get(&self.local_peer_id) {
                        None => {
                            log::warn!("This node is not included in the topology");
                            return Poll::Ready(NetworkBehaviourAction::GenerateEvent(
                                Event::LocalRemoved,
                            ));
                        }
                        Some(peer) => {
                            waiting_dial
                                .connected_peers
                                .insert(self.local_peer_id, peer.clone());
                        }
                    }

                    waiting_dial.waiting_to_dial =
                        waiting_dial.new_topology.values().cloned().collect();

                    waiting_dial
                        .waiting_to_dial
                        .retain(|peer| self.local_peer_id != peer.peer_id);

                    self.state = State::WaitingDial(waiting_dial);

                    Poll::Ready(NetworkBehaviourAction::GenerateEvent(
                        Event::PeerUpdatePending,
                    ))
                } else {
                    Poll::Pending
                }
            }
            State::WaitingDial(WaitingDial {
                new_topology: _,
                connected_peers: _,
                waiting_to_dial: waiting_dial,
                dial_failed,
                dial_attempts,
                interval_between_dial_attempts,
            }) => match waiting_dial.pop() {
                Some(peer) => {
                    log::trace!("Dialing peer: {:?}", peer.peer_id);

                    let opts = DialOpts::peer_id(*peer.peer_id.inner())
                        .condition(PeerCondition::NotDialing)
                        .addresses(vec![peer.address.clone()])
                        .build();

                    Poll::Ready(NetworkBehaviourAction::Dial { opts })
                }
                None => {
                    if *dial_attempts >= MAX_DIAL_ATTEMPT_ROUNDS {
                        log::warn!("Failed to establish enough connections to new topology");

                        self.state = State::WaitingPeers;
                        return Poll::Pending;
                    } else {
                        if interval_between_dial_attempts.is_none() {
                            let start_at = time::Instant::now() + Duration::from_secs(5);
                            *interval_between_dial_attempts =
                                Some(time::interval_at(start_at, Duration::from_secs(10)));
                        }

                        if let Some(interval) = interval_between_dial_attempts {
                            if interval.poll_tick(cx) == Poll::Pending {
                                return Poll::Pending;
                            } else {
                                *dial_attempts += 1;
                                log::trace!("Next attempt({dial_attempts:?}) to dial failed peers");
                            }
                        }

                        if *dial_attempts > 0 {
                            log::trace!("Dialing failed peers size: {:?}", dial_failed.len());
                            waiting_dial.extend(dial_failed.clone().into_iter());
                            dial_failed.clear();
                        }
                    }
                    Poll::Pending
                }
            },
            State::NotifyPeersUpdated(NotifyPeersUpdated { new_topology }) => {
                self.peers = new_topology.clone();
                self.state = State::WaitingPeers;
                Poll::Ready(NetworkBehaviourAction::GenerateEvent(Event::PeersUpdated))
            }
        }
    }
}
