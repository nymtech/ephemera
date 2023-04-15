use std::collections::HashSet;
use std::fmt::{Debug, Formatter};
use std::{
    collections::HashMap,
    task::{Context, Poll},
    time::Duration,
};

use libp2p::swarm::ToSwarm;
use libp2p::{
    core::ConnectedPoint,
    swarm::{
        behaviour::ConnectionEstablished,
        dial_opts::{DialOpts, PeerCondition},
        ConnectionClosed, ConnectionId, DialFailure, FromSwarm, NetworkBehaviour, PollParameters,
        THandlerInEvent, THandlerOutEvent,
    },
    Multiaddr,
};
use serde_derive::Serialize;
use tokio::{task::JoinHandle, time};

use crate::network::libp2p::behaviours::peer_discovery::{
    calculate_minimum_available_nodes, MAX_DIAL_ATTEMPT_ROUNDS,
};
use crate::network::{
    discovery::{PeerDiscovery, PeerInfo},
    libp2p::behaviours::peer_discovery::handler::Handler,
    peer::{Peer, PeerId},
};

#[derive(PartialEq, Eq, Debug, Clone, Hash, Serialize)]
enum Endpoint {
    Dialer {
        address: String,
    },
    Listener {
        local_address: String,
        remote_address: String,
    },
}

impl From<ConnectedPoint> for Endpoint {
    fn from(connected_point: ConnectedPoint) -> Self {
        match connected_point {
            ConnectedPoint::Dialer { address, .. } => Endpoint::Dialer {
                address: address.to_string(),
            },
            ConnectedPoint::Listener {
                local_addr,
                send_back_addr,
            } => Endpoint::Listener {
                local_address: local_addr.to_string(),
                remote_address: send_back_addr.to_string(),
            },
        }
    }
}

#[derive(Default, Serialize)]
struct ConnectedPeers {
    dialer: HashMap<libp2p_identity::PeerId, HashSet<Endpoint>>,
    listener: HashMap<libp2p_identity::PeerId, HashSet<Endpoint>>,
}

impl ConnectedPeers {
    fn new() -> Self {
        ConnectedPeers {
            dialer: HashMap::new(),
            listener: HashMap::new(),
        }
    }

    fn insert(&mut self, peer_id: libp2p_identity::PeerId, connected_point: Endpoint) {
        match connected_point {
            Endpoint::Dialer { .. } => {
                self.dialer
                    .entry(peer_id)
                    .or_default()
                    .insert(connected_point);
            }
            Endpoint::Listener { .. } => {
                self.listener
                    .entry(peer_id)
                    .or_default()
                    .insert(connected_point);
            }
        }
    }

    fn remove(&mut self, peer_id: &libp2p_identity::PeerId, connected_point: &Endpoint) {
        match connected_point {
            Endpoint::Dialer { .. } => {
                if let Some(connected_points) = self.dialer.get_mut(peer_id) {
                    connected_points.remove(connected_point.into());
                    if connected_points.is_empty() {
                        self.dialer.remove(peer_id);
                    }
                }
            }
            Endpoint::Listener { .. } => {
                if let Some(connected_points) = self.listener.get_mut(peer_id) {
                    connected_points.remove(connected_point);
                    if connected_points.is_empty() {
                        self.listener.remove(peer_id);
                    }
                }
            }
        }
    }
}

impl Debug for ConnectedPeers {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let json = serde_json::json!(&self);
        match serde_json::to_string_pretty(&json) {
            Ok(s) => write!(f, "{}", s),
            Err(_) => f
                .debug_struct("ConnectedPeers")
                .field("dialer", &self.dialer)
                .field("listener", &self.listener)
                .finish(),
        }
    }
}

/// PeerDiscovery state when we are trying to connect to new peers.
#[derive(Debug, Default)]
struct WaitingDial {
    /// All peers what were provided by the discovery trait.
    new_topology: HashMap<PeerId, Peer>,
    /// Peers that we successfully connected to.
    connected_peers: HashMap<PeerId, Peer>,
    /// Peers that we are haven't tried to connect to yet.
    waiting_to_dial: Vec<Peer>,
    /// Peers that we tried to connect to but failed.
    dial_failed: Vec<Peer>,
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
    WaitingDial(WaitingDial),
    /// We have finished trying to connect to new peers and going to report it.
    NotifyPeersUpdated(NotifyPeersUpdated),
}

#[derive(Debug, Default)]
struct NotifyPeersUpdated {
    new_topology: HashMap<PeerId, Peer>,
}

pub(crate) struct Behaviour<P: PeerDiscovery> {
    /// All peers that are part of the current topology.
    peers: HashMap<PeerId, Peer>,
    /// Peers that were part of the previous topology.
    previous_peers: HashMap<PeerId, Peer>,
    /// Local peer id.
    local_peer_id: PeerId,
    /// Peer discovery trait. It's an external trait that provides us with new peers.
    peer_discovery: Option<P>,
    /// Channel to send new peers to the PeerDiscovery trait.
    discovery_channel_tx: tokio::sync::mpsc::UnboundedSender<Vec<PeerInfo>>,
    /// Channel to receive new peers from the PeerDiscovery trait.
    discovery_channel_rcv: tokio::sync::mpsc::UnboundedReceiver<Vec<PeerInfo>>,
    /// Current state of the behaviour.
    state: State,
    /// Current state of all incoming and outgoing connections.
    all_connections: ConnectedPeers,
}

impl<P: PeerDiscovery + 'static> Behaviour<P> {
    pub(crate) fn new(peer_discovery: P, local_peer_id: PeerId) -> Self {
        let (tx, rcv) = tokio::sync::mpsc::unbounded_channel();
        Behaviour {
            peers: Default::default(),
            local_peer_id,
            previous_peers: Default::default(),
            peer_discovery: Some(peer_discovery),
            discovery_channel_tx: tx,
            discovery_channel_rcv: rcv,
            state: State::WaitingPeers,
            all_connections: Default::default(),
        }
    }

    /// Returns the list of peers that are part of current topology.
    pub(crate) fn peer_ids(&self) -> Vec<PeerId> {
        self.peers.keys().cloned().collect()
    }

    /// Returns the list of peers that are part of current topology.
    pub(crate) fn peers(&self) -> Vec<Peer> {
        self.peers.values().cloned().collect()
    }

    /// Returns the list of peers that were part of previous topology.
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
                if endpoint.is_dialer() {
                    if let State::WaitingDial(WaitingDial {
                        new_topology,
                        connected_peers,
                        waiting_to_dial: _,
                        dial_failed,
                        dial_attempts: _,
                        interval_between_dial_attempts: _,
                    }) = &mut self.state
                    {
                        let ephemera_id = peer_id.into();
                        if let Some(peer) = new_topology.get(&ephemera_id) {
                            connected_peers.insert(ephemera_id, peer.clone());
                            dial_failed.retain(|peer| peer.peer_id != ephemera_id);
                        }
                    }
                }

                let endpoint = endpoint.clone().into();
                self.all_connections.insert(peer_id, endpoint);
                log::debug!("{:?}", self.all_connections);
            }

            FromSwarm::ConnectionClosed(ConnectionClosed {
                peer_id,
                connection_id: _,
                endpoint,
                handler: _,
                remaining_established: _,
            }) => {
                let endpoint = endpoint.clone().into();

                self.all_connections.remove(&peer_id, &endpoint);
                log::debug!("{:?}", self.all_connections);
            }
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
    ) -> Poll<ToSwarm<Self::OutEvent, THandlerInEvent<Self>>> {
        match &mut self.state {
            State::WaitingPeers => {
                if let Poll::Ready(Some(peers)) = self.discovery_channel_rcv.poll_recv(cx) {
                    log::info!("Received peers from discovery: {:?}", peers);
                    if peers.is_empty() {
                        //Not sure what to do here. Tempted to think that if this happens
                        //we should ignore it and assume that this is a bug in the discovery service.
                        log::warn!("Received empty peers from discovery. To try again before preconfigured interval, please restart the node.");
                        return Poll::Pending;
                    }

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
                            return Poll::Ready(ToSwarm::GenerateEvent(Event::LocalRemoved));
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

                    Poll::Ready(ToSwarm::GenerateEvent(Event::PeerUpdatePending))
                } else {
                    Poll::Pending
                }
            }
            State::WaitingDial(WaitingDial {
                new_topology,
                connected_peers,
                waiting_to_dial: waiting_dial,
                dial_failed,
                dial_attempts,
                interval_between_dial_attempts,
            }) => match waiting_dial.pop() {
                Some(peer) => {
                    log::debug!("Dialing peer: {:?}", peer.peer_id);

                    let opts = DialOpts::peer_id(*peer.peer_id.inner())
                        .condition(PeerCondition::NotDialing)
                        .addresses(vec![peer.address.inner()])
                        .build();

                    Poll::Ready(ToSwarm::Dial { opts })
                }
                None => {
                    if connected_peers.len() == new_topology.len()
                        || *dial_attempts >= MAX_DIAL_ATTEMPT_ROUNDS
                    {
                        let minimum_available_nodes =
                            calculate_minimum_available_nodes(new_topology.len());

                        log::debug!(
                            "Connected to {} peers, minimum required nodes: {}",
                            connected_peers.len(),
                            minimum_available_nodes
                        );

                        if connected_peers.len() >= minimum_available_nodes {
                            let notify_peers_updated = NotifyPeersUpdated {
                                new_topology: connected_peers.clone(),
                            };
                            self.state = State::NotifyPeersUpdated(notify_peers_updated);
                        } else {
                            log::warn!("Failed to establish enough connections to new topology");

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
                                log::debug!("Next attempt({dial_attempts:?}) to dial failed peers");
                            }
                        }

                        if !dial_failed.is_empty() {
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
                Poll::Ready(ToSwarm::GenerateEvent(Event::PeersUpdated))
            }
        }
    }
}
