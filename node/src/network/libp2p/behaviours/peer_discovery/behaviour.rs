//! # Peer discovery behaviour.
//!
//! Ephemera `reliable broadcast` needs to know the list of peers who participate in the protocol.
//! Also it's not enough to have just the list but to make sure that they are actually online.
//! When the list of available peers changes, `reliable broadcast` needs to be notified so that it can adjust accordingly.
//!
//! This behaviour is responsible for discovering peers and keeping the list of peers up to date.
//!
//! User provides a [PeerDiscovery] trait implementation to the [Behaviour]. It is responsible for fetching the list of peers.
//!
//! [Behaviour] accepts only peers that are actually online.
//!
//! When peers become available or unavailable, [Behaviour] adjusts the list of connected peers accordingly and notifies `reliable broadcast`.
//!
//! It is configurable what `threshold` of peers(from the total list provided by [PeerDiscovery]) should be available at any given time.
//!
//! Ideally [PeerDiscovery] depends on a resource that gives reliable results. Some kind of registry which itself keeps track of actually online nodes.
//! As Ephemera uses only peers provided by [PeerDiscovery], it should be able to rely on it.

use std::collections::HashSet;
use std::fmt::{Debug, Formatter};
use std::{
    collections::HashMap,
    task::{Context, Poll},
    time::Duration,
};

use libp2p::{
    core::ConnectedPoint,
    swarm::ToSwarm,
    swarm::{
        behaviour::ConnectionEstablished,
        dial_opts::{DialOpts, PeerCondition},
        ConnectionClosed, ConnectionId, DialFailure, FromSwarm, NetworkBehaviour, PollParameters,
        THandlerInEvent, THandlerOutEvent,
    },
    Multiaddr,
};
use log::{debug, error, info, trace, warn};
use serde_derive::Serialize;
use tokio::{task::JoinHandle, time};

use crate::network::libp2p::behaviours::peer_discovery::MEMBERSHIP_MINIMUM_AVAILABLE_NODES_RATIO;
use crate::{
    logging::pretty_json,
    network::{
        discovery::{PeerDiscovery, PeerInfo},
        libp2p::behaviours::peer_discovery::{handler::Handler, MAX_DIAL_ATTEMPT_ROUNDS},
        peer::{Peer, PeerId},
    },
};

#[derive(Debug)]
enum MembershipKind {
    /// Specified threshold of peers(from total provided by [PeerDiscovery]) need to be available.
    /// Threshold value is defined the ratio of peers that need to be available.
    /// For example, if the threshold is 0.5, then at least 50% of the peers need to be available.
    Threshold(f64),
    /// Membership is defined by peers who are online.
    AllOnline,
}

impl MembershipKind {
    pub(crate) fn accept(&self, connected_peers: usize, total_number_of_peers: usize) -> bool {
        match self {
            MembershipKind::Threshold(threshold) => {
                let minimum_available_nodes = (total_number_of_peers as f64 * threshold) as usize;
                connected_peers >= minimum_available_nodes
            }
            MembershipKind::AllOnline => connected_peers == total_number_of_peers,
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Hash, Serialize)]
enum Endpoint {
    Dialer {
        address: Multiaddr,
    },
    Listener {
        local_address: Multiaddr,
        remote_address: Multiaddr,
    },
}

impl From<ConnectedPoint> for Endpoint {
    fn from(connected_point: ConnectedPoint) -> Self {
        match connected_point {
            ConnectedPoint::Dialer { address, .. } => Endpoint::Dialer { address },
            ConnectedPoint::Listener {
                local_addr,
                send_back_addr,
            } => Endpoint::Listener {
                local_address: local_addr,
                remote_address: send_back_addr,
            },
        }
    }
}

#[derive(Default, Serialize)]
struct Connections {
    dialer: HashSet<Endpoint>,
    listener: HashSet<Endpoint>,
}

impl Connections {
    fn insert(&mut self, connected_point: Endpoint) {
        match connected_point {
            Endpoint::Dialer { .. } => {
                self.dialer.insert(connected_point);
            }
            Endpoint::Listener { .. } => {
                self.listener.insert(connected_point);
            }
        }
    }

    fn remove(&mut self, connected_point: &Endpoint) {
        match connected_point {
            Endpoint::Dialer { .. } => {
                self.dialer.remove(connected_point);
            }
            Endpoint::Listener { .. } => {
                self.listener.remove(connected_point);
            }
        }
    }
}

#[derive(Default, Serialize)]
struct ConnectedPeers {
    connections: HashMap<libp2p_identity::PeerId, Connections>,
}

impl ConnectedPeers {
    fn insert(&mut self, peer_id: libp2p_identity::PeerId, connected_point: Endpoint) {
        let connections = self.connections.entry(peer_id).or_default();
        connections.insert(connected_point);
    }

    fn remove(&mut self, peer_id: &libp2p_identity::PeerId, connected_point: &Endpoint) {
        if let Some(connections) = self.connections.get_mut(peer_id) {
            connections.remove(connected_point);
        }
    }
}

impl Debug for ConnectedPeers {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let output = pretty_json(self);
        write!(f, "{}", output)
    }
}

/// PeerDiscovery state when we are trying to connect to new peers.
#[derive(Debug, Default)]
struct PendingPeersUpdate {
    /// All peers what were provided by the discovery trait.
    new_membership: HashMap<libp2p_identity::PeerId, Peer>,
    /// Peers that we successfully connected to.
    connected_peers: HashSet<libp2p_identity::PeerId>,
    /// Peers that we are haven't tried to connect to yet.
    waiting_to_dial: HashSet<libp2p_identity::PeerId>,
    /// Peers that we tried to connect to but failed.
    dial_failed: HashSet<libp2p_identity::PeerId>,
    /// Number of dial attempts per round.
    dial_attempts: usize,
    /// How long we wait between dial attempts.
    interval_between_dial_attempts: Option<time::Interval>,
}

/// PeerDiscovery behaviour states.
enum State {
    /// Waiting for new peers from the discovery trait.
    WaitingPeers,
    /// Trying to connect to new peers.
    WaitingDial(PendingPeersUpdate),
    /// We have finished trying to connect to new peers and going to report it.
    NotifyPeersUpdated(NotifyPeersUpdated),
}

#[derive(Debug, Default)]
struct NotifyPeersUpdated {
    peers: Vec<Peer>,
}

impl NotifyPeersUpdated {
    fn new(peers: Vec<Peer>) -> Self {
        NotifyPeersUpdated { peers }
    }
}

pub(crate) struct Behaviour<P: PeerDiscovery> {
    /// All peers that are part of the current group.
    peers: HashMap<PeerId, Peer>,
    /// Peers that were part of the previous group.
    previous_peers: HashMap<PeerId, Peer>,
    /// Local peer id.
    local_peer_id: PeerId,
    /// Peer discovery trait. It's an external trait that provides us with new peers.
    peer_discovery: Option<P>,
    /// Interval to check for new peers. It's the value returned by the discovery trait[PeerDiscovery::get_poll_interval].
    peer_discovery_interval_sec: u64,
    /// Channel to send new peers to the PeerDiscovery trait.
    discovery_channel_tx: tokio::sync::mpsc::UnboundedSender<Vec<PeerInfo>>,
    /// Channel to receive new peers from the PeerDiscovery trait.
    discovery_channel_rcv: tokio::sync::mpsc::UnboundedReceiver<Vec<PeerInfo>>,
    /// Current state of the behaviour.
    state: State,
    /// Current state of all incoming and outgoing connections.
    all_connections: ConnectedPeers,
    /// Membership kind.
    membership_kind: MembershipKind,
}

impl<P: PeerDiscovery + 'static> Behaviour<P> {
    pub(crate) fn new(peer_discovery: P, local_peer_id: PeerId) -> Self {
        let (tx, rcv) = tokio::sync::mpsc::unbounded_channel();
        Behaviour {
            peers: Default::default(),
            local_peer_id,
            previous_peers: Default::default(),
            peer_discovery: Some(peer_discovery),
            peer_discovery_interval_sec: 0,
            discovery_channel_tx: tx,
            discovery_channel_rcv: rcv,
            state: State::WaitingPeers,
            all_connections: Default::default(),
            membership_kind: MembershipKind::Threshold(MEMBERSHIP_MINIMUM_AVAILABLE_NODES_RATIO),
        }
    }

    /// Returns the list of peers that are part of current group.
    pub(crate) fn peer_ids(&self) -> Vec<PeerId> {
        self.peers.keys().cloned().collect()
    }

    /// Returns the list of peers that are part of current group.
    pub(crate) fn peers(&self) -> Vec<Peer> {
        self.peers.values().cloned().collect()
    }

    /// Returns the list of peers that were part of previous group.
    pub(crate) fn previous_peer_ids(&self) -> Vec<PeerId> {
        self.previous_peers.keys().cloned().collect()
    }

    /// Starts to poll the peer discovery trait.
    pub(crate) async fn spawn_peer_discovery(&mut self) -> anyhow::Result<JoinHandle<()>> {
        let tx = self.discovery_channel_tx.clone();
        let mut peer_discovery = self
            .peer_discovery
            .take()
            .ok_or(anyhow::anyhow!("Peer discovery already spawned"))?;

        self.peer_discovery_interval_sec = peer_discovery.get_poll_interval().as_secs();

        let join_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(peer_discovery.get_poll_interval());
            loop {
                interval.tick().await;
                trace!("Polling peer discovery (after tick)");
                if let Err(err) = peer_discovery.poll(tx.clone()).await {
                    error!("Error while polling peer discovery: {}", err);
                }
            }
        });
        Ok(join_handle)
    }
}

/// Events that can be emitted by the `Behaviour`.
pub(crate) enum Event {
    /// We have received new peers from the discovery trait.
    /// We are going to try to connect to them.
    PeerUpdatePending,
    /// We have finished trying to connect to new peers and going to report it.
    PeersUpdated,
    /// PeerDiscovery trait reported us new peers and this set doesn't contain our local peer.
    LocalRemoved,
    /// PeerDiscovery trait reported us new peers and we failed to connect to enough of them.
    NotEnoughPeers,
}

impl<P: PeerDiscovery + 'static> NetworkBehaviour for Behaviour<P> {
    type ConnectionHandler = Handler;
    type OutEvent = Event;

    fn new_handler(&mut self) -> Self::ConnectionHandler {
        Handler
    }

    ///Peer discovery behaviour is responsible for providing addresses to another Swarm behaviours.
    fn addresses_of_peer(&mut self, peer_id: &libp2p_identity::PeerId) -> Vec<Multiaddr> {
        self.peers
            .get(&PeerId(*peer_id))
            .map(|peer| vec![peer.address.inner()])
            .unwrap_or_default()
    }

    fn on_swarm_event(&mut self, event: FromSwarm<Self::ConnectionHandler>) {
        match event {
            FromSwarm::ConnectionEstablished(ConnectionEstablished {
                peer_id,
                connection_id: _,
                endpoint,
                failed_addresses: _,
                other_established: _,
            }) => {
                if let State::WaitingDial(PendingPeersUpdate {
                    new_membership: new_group,
                    connected_peers,
                    waiting_to_dial: _,
                    dial_failed,
                    dial_attempts: _,
                    interval_between_dial_attempts: _,
                }) = &mut self.state
                {
                    if new_group.contains_key(&peer_id) {
                        connected_peers.insert(peer_id);
                        dial_failed.retain(|id| *id != peer_id);
                    }
                }

                self.all_connections
                    .insert(peer_id, endpoint.clone().into());
                debug!("{:?}", self.all_connections);
            }

            FromSwarm::ConnectionClosed(ConnectionClosed {
                peer_id,
                connection_id: _,
                endpoint,
                handler: _,
                remaining_established: _,
            }) => {
                self.all_connections
                    .remove(&peer_id, &endpoint.clone().into());
                debug!("{:?}", self.all_connections);
            }
            FromSwarm::DialFailure(DialFailure {
                peer_id: Some(peer_id),
                error,
                connection_id: _,
            }) => {
                if let State::WaitingDial(PendingPeersUpdate {
                    new_membership: _,
                    connected_peers: _,
                    waiting_to_dial: _,
                    dial_failed,
                    dial_attempts: _,
                    interval_between_dial_attempts: _,
                }) = &mut self.state
                {
                    trace!("DialFailure, peer_id: {peer_id:?}, error: {error:?}",);

                    if !self.all_connections.connections.contains_key(&peer_id) {
                        dial_failed.insert(peer_id);
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
    ) -> Poll<ToSwarm<Self::OutEvent, THandlerInEvent<Self>>> {
        match &mut self.state {
            State::WaitingPeers => {
                if let Poll::Ready(Some(peers)) = self.discovery_channel_rcv.poll_recv(cx) {
                    info!("Received peers from discovery: {:?}", peers);
                    if peers.is_empty() {
                        //Not sure what to do here. Tempted to think that if this happens
                        //we should ignore it and assume that this is a bug in the discovery service.
                        warn!("Received empty peers from discovery. To try again before preconfigured interval, please restart the node.");
                        return Poll::Pending;
                    }

                    self.previous_peers = std::mem::take(&mut self.peers);

                    let mut pending_update = PendingPeersUpdate::default();

                    for peer_info in peers {
                        match <PeerInfo as TryInto<Peer>>::try_into(peer_info) {
                            Ok(peer) => {
                                pending_update
                                    .new_membership
                                    .insert(*peer.peer_id.inner(), peer.clone());
                            }
                            Err(err) => {
                                error!("Error while converting peer info to peer: {}", err);
                            }
                        }
                    }

                    if pending_update
                        .new_membership
                        .contains_key(self.local_peer_id.inner())
                    {
                        let local_id = *self.local_peer_id.inner();
                        pending_update.connected_peers.insert(local_id);

                        pending_update.waiting_to_dial =
                            pending_update.new_membership.keys().cloned().collect();

                        pending_update
                            .waiting_to_dial
                            .retain(|peer| local_id != *peer);

                        for peer_id in self.all_connections.connections.keys() {
                            pending_update.waiting_to_dial.retain(|id| peer_id != id);
                            if pending_update.new_membership.contains_key(peer_id) {
                                pending_update.connected_peers.insert(*peer_id);
                            }
                        }

                        if !pending_update.waiting_to_dial.is_empty() {
                            self.state = State::WaitingDial(pending_update);
                            Poll::Ready(ToSwarm::GenerateEvent(Event::PeerUpdatePending))
                        } else {
                            let peers = pending_update
                                .new_membership
                                .into_iter()
                                .filter(|(id, _)| pending_update.connected_peers.contains(id))
                                .map(|(_, peer)| peer)
                                .collect();
                            self.state = State::NotifyPeersUpdated(NotifyPeersUpdated::new(peers));
                            Poll::Pending
                        }
                    } else {
                        warn!("This node is not included in the broadcast group");
                        Poll::Ready(ToSwarm::GenerateEvent(Event::LocalRemoved))
                    }
                } else {
                    Poll::Pending
                }
            }
            State::WaitingDial(PendingPeersUpdate {
                new_membership: new_group,
                connected_peers,
                waiting_to_dial,
                dial_failed,
                dial_attempts,
                interval_between_dial_attempts,
            }) => {
                //Refresh the list of connected peers
                for peer_id in self.all_connections.connections.keys() {
                    waiting_to_dial.retain(|id| peer_id != id);
                    if new_group.contains_key(peer_id) {
                        connected_peers.insert(*peer_id);
                    }
                }

                let next_waiting = waiting_to_dial.iter().next().cloned();

                match next_waiting {
                    Some(peer_id) => {
                        waiting_to_dial.remove(&peer_id);
                        let address = new_group.get(&peer_id).unwrap().address.inner();

                        let opts = DialOpts::peer_id(peer_id)
                            .condition(PeerCondition::NotDialing)
                            .addresses(vec![address])
                            .build();

                        debug!("Dialing peer: {:?}", peer_id);
                        Poll::Ready(ToSwarm::Dial { opts })
                    }
                    None => {
                        let all_connected = connected_peers.len() == new_group.len();
                        if all_connected || *dial_attempts >= MAX_DIAL_ATTEMPT_ROUNDS {
                            let membership_accept = self
                                .membership_kind
                                .accept(connected_peers.len(), new_group.len());

                            if membership_accept {
                                let mut new_peers = HashMap::new();
                                for peer_id in connected_peers.iter() {
                                    new_peers.insert(
                                        (*peer_id).into(),
                                        new_group.get(peer_id).unwrap().clone(),
                                    );
                                }

                                self.peers = new_peers;
                                self.state = State::WaitingPeers;

                                return Poll::Ready(ToSwarm::GenerateEvent(Event::PeersUpdated));
                            } else {
                                warn!("Failed to establish enough connections to new peers.");

                                interval_between_dial_attempts.take();
                                self.state = State::WaitingPeers;
                                return Poll::Ready(ToSwarm::GenerateEvent(Event::NotEnoughPeers));
                            }
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
                                    trace!("Next attempt({dial_attempts:?}) to dial failed peers");
                                }
                            }

                            if !dial_failed.is_empty() {
                                trace!("Dialing failed peers size: {:?}", dial_failed.len());
                                waiting_to_dial.extend(dial_failed.clone().into_iter());
                                dial_failed.clear();
                            }
                        }
                        Poll::Pending
                    }
                }
            }
            State::NotifyPeersUpdated(NotifyPeersUpdated { peers }) => {
                self.peers = peers
                    .iter()
                    .map(|peer| (peer.peer_id, peer.clone()))
                    .collect();
                self.state = State::WaitingPeers;
                Poll::Ready(ToSwarm::GenerateEvent(Event::PeersUpdated))
            }
        }
    }
}
