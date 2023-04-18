//! # Peer discovery behaviour.
//!
//! Ephemera `reliable broadcast` needs to know the list of peers who participate in the protocol.
//! Also it's not enough to have just the list but to make sure that they are actually online.
//! When the list of available peers changes, `reliable broadcast` needs to be notified so that it can adjust accordingly.
//!
//! This behaviour is responsible for discovering peers and keeping the list of peers up to date.
//!
//! User provides a [PeerDiscovery] trait implementation to the [Behaviour] which is responsible for fetching the list of peers.
//!
//! [Behaviour] accepts only peers that are actually online.
//!
//! When peers become available or unavailable, [Behaviour] adjusts the list of connected peers accordingly and notifies `reliable broadcast`
//! about the membership change.
//!
//! It is configurable what `threshold` of peers(from the total list provided by [PeerDiscovery]) should be available at any given time.
//! Or if just to use all peers who are online. See [MembershipKind] for more details.
//!
//! Ideally [PeerDiscovery] can depend on a resource that gives reliable results. Some kind of registry which itself keeps track of actually online nodes.
//! As Ephemera uses only peers provided by [PeerDiscovery], it depends on its accuracy.
//! At the same time it tries to be flexible and robust to handle less reliable [PeerDiscovery] implementations.

// When peer gets disconnected, we try to dial it and if that fails, we update group.
// (it may connect us meanwhile).
// Although we can retry to connect to disconnected peers, it's simpler if we just assume that when
// they come online again, they will connect us.

//So the main goal is to remove offline peers from the group so that rb can make progress.

//a)when peer disconnects, we try to dial it and if that fails, we update group.
//b)when peer connects, we will update the group.

use std::time::Duration;
use std::{
    collections::HashMap,
    collections::HashSet,
    fmt::Debug,
    task::{Context, Poll},
};

use libp2p::core::Endpoint;
use libp2p::swarm::ConnectionDenied;
use libp2p::{
    swarm::ToSwarm,
    swarm::{
        behaviour::ConnectionEstablished,
        dial_opts::{DialOpts, PeerCondition},
        ConnectionClosed, ConnectionId, DialFailure, FromSwarm, NetworkBehaviour, PollParameters,
        THandlerInEvent, THandlerOutEvent,
    },
    Multiaddr,
};
use libp2p_identity::PeerId;
use log::{debug, error, trace, warn};
use tokio::{task::JoinHandle, time};

use crate::network::libp2p::behaviours::peer_discovery::peers_requester::PeersRequester;
use crate::peer::Peer;
use crate::{
    network::libp2p::behaviours::{
        peer_discovery::connections::ConnectedPeers,
        peer_discovery::membership::MembershipKind,
        peer_discovery::membership::{Membership, Memberships},
        peer_discovery::MEMBERSHIP_MINIMUM_AVAILABLE_NODES_RATIO,
        peer_discovery::{handler::Handler, MAX_DIAL_ATTEMPT_ROUNDS},
    },
    peer_discovery::{PeerDiscovery, PeerInfo},
};

/// PeerDiscovery state when we are trying to connect to new peers.
///
/// We try to connect few times before giving up. Generally speaking an another peer is either online or offline
/// at any given time. But it has been helpful for testing when whole cluster comes up around the same time.
#[derive(Debug, Default)]
struct PendingPeersUpdate {
    /// Peers that we are haven't tried to connect to yet.
    waiting_to_dial: HashSet<PeerId>,
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
    NotifyPeersUpdated,
}

/// Events that can be emitted by the `Behaviour`.
pub(crate) enum Event {
    /// We have received new peers from the discovery trait.
    /// We are going to try to connect to them.
    PeerUpdatePending,
    /// We have finished trying to connect to new peers and going to report it.
    PeersUpdated(HashSet<PeerId>),
    /// PeerDiscovery trait reported us new peers and this set doesn't contain our local peer.
    LocalRemoved,
    /// PeerDiscovery trait reported us new peers and we failed to connect to enough of them.
    NotEnoughPeers,
}

pub(crate) struct Behaviour<P: PeerDiscovery> {
    /// All peers that are part of the current group.
    memberships: Memberships,
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
    /// Membership kind.
    membership_kind: MembershipKind,
}

impl<'a, P: PeerDiscovery + 'static> Behaviour<P> {
    pub(crate) fn new(peer_discovery: P, local_peer_id: PeerId) -> Self {
        let (tx, rcv) = tokio::sync::mpsc::unbounded_channel();
        Behaviour {
            memberships: Memberships::new(),
            local_peer_id,
            peer_discovery: Some(peer_discovery),
            discovery_channel_tx: tx,
            discovery_channel_rcv: rcv,
            state: State::WaitingPeers,
            all_connections: Default::default(),
            membership_kind: MembershipKind::Threshold(MEMBERSHIP_MINIMUM_AVAILABLE_NODES_RATIO),
        }
    }

    /// Returns the list of peers that are part of current group.
    pub(crate) fn active_peer_ids(&mut self) -> HashSet<&PeerId> {
        self.memberships.current().active_peer_ids_ref()
    }

    pub(crate) fn active_peer_ids_with_local(&mut self) -> HashSet<PeerId> {
        self.memberships.current().active_peer_ids_with_local()
    }

    /// Starts to poll the peer discovery trait.
    pub(crate) async fn spawn_peer_discovery(&mut self) -> anyhow::Result<JoinHandle<()>> {
        let peer_discovery = self
            .peer_discovery
            .take()
            .ok_or(anyhow::anyhow!("Peer discovery already spawned"))?;

        let join_handle =
            PeersRequester::spawn_peer_requester(peer_discovery, self.discovery_channel_tx.clone())
                .await?;

        Ok(join_handle)
    }
}

impl<P: PeerDiscovery + 'static> NetworkBehaviour for Behaviour<P> {
    type ConnectionHandler = Handler;
    type OutEvent = Event;

    fn new_handler(&mut self) -> Self::ConnectionHandler {
        Handler
    }

    fn handle_pending_inbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        _local_addr: &Multiaddr,
        _remote_addr: &Multiaddr,
    ) -> Result<(), ConnectionDenied> {
        //TODO: we can refuse connections from peers that are not part of the current membership.
        Ok(())
    }

    fn handle_pending_outbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        maybe_peer: Option<PeerId>,
        _addresses: &[Multiaddr],
        _effective_role: Endpoint,
    ) -> Result<Vec<Multiaddr>, ConnectionDenied> {
        match maybe_peer {
            Some(peer_id) => Ok(self.addresses_of_peer(&peer_id)),
            None => Ok(vec![]),
        }
    }

    ///`Peer discovery` behaviour is responsible for providing addresses to another Swarm behaviours.
    fn addresses_of_peer(&mut self, peer_id: &PeerId) -> Vec<Multiaddr> {
        self.memberships
            .current()
            .peer_address(peer_id)
            .cloned()
            .map_or(vec![], |addr| vec![addr])
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
                self.all_connections
                    .insert(peer_id, endpoint.clone().into());
                if let Some(pending) = self.memberships.pending_mut() {
                    pending.add_active_peer(peer_id);
                }
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
                trace!("Dial failure: {:?} {:?}", peer_id, error);
            }
            _ => {}
        }
    }

    //Currently this behaviour doesn't do any direct networking with other peers itself.
    fn on_connection_handler_event(
        &mut self,
        _peer_id: PeerId,
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
                    debug!("Received peers from peer discovery: {:?}", peers);

                    if peers.is_empty() {
                        //Not sure what to do here. Tempted to think that if this happens
                        //we should ignore it and assume that this is a bug in the discovery service.

                        warn!("Received empty peers from discovery. To try again before preconfigured interval, please restart the node.");
                        return Poll::Ready(ToSwarm::GenerateEvent(Event::NotEnoughPeers));
                    }

                    let mut new_peers = HashMap::new();

                    for peer_info in peers {
                        match <PeerInfo as TryInto<Peer>>::try_into(peer_info) {
                            Ok(peer) => {
                                // pending_membership.add_peer(peer);
                                // pending_update.new_membership.insert(*peer.peer_id.inner(), &peer);
                                new_peers.insert(*peer.peer_id.inner(), peer);
                            }
                            Err(err) => {
                                error!("Error while converting peer info to peer: {}", err);
                            }
                        }
                    }

                    //If we are not part of the new membership, notify immediately
                    if !new_peers.contains_key(&self.local_peer_id) {
                        self.state = State::NotifyPeersUpdated;
                        return Poll::Pending;
                    } else {
                        new_peers.remove(&self.local_peer_id);
                    }

                    let mut pending_membership =
                        Membership::new_with_local(new_peers.clone(), self.local_peer_id);
                    let mut pending_update = PendingPeersUpdate::default();

                    for peer_id in new_peers.keys() {
                        if self.all_connections.is_peer_connected(peer_id) {
                            pending_membership.add_active_peer(*peer_id);
                        } else {
                            pending_update.waiting_to_dial.insert(*peer_id);
                        }
                    }

                    self.memberships.set_pending(pending_membership);

                    //It seems that all peers from updated membership set are already connected
                    if pending_update.waiting_to_dial.is_empty() {
                        self.state = State::NotifyPeersUpdated;
                        Poll::Pending
                    } else {
                        self.state = State::WaitingDial(pending_update);

                        //Just let the rest of the system to know that we are in the middle of updating membership
                        Poll::Ready(ToSwarm::GenerateEvent(Event::PeerUpdatePending))
                    }
                } else {
                    Poll::Pending
                }
            }
            State::WaitingDial(PendingPeersUpdate {
                waiting_to_dial,
                dial_attempts,
                interval_between_dial_attempts,
            }) => {
                //Refresh the list of connected peers
                for peer_id in self.all_connections.all_connected_peers_ref() {
                    waiting_to_dial.remove(peer_id);
                }

                let pending_membership = self
                    .memberships
                    .pending()
                    .expect("Pending membership should be set");

                //With each 'poll' we can tell Swarm to dial one peer
                let next_waiting = waiting_to_dial.iter().next().cloned();

                match next_waiting {
                    Some(peer_id) => {
                        waiting_to_dial.remove(&peer_id);
                        println!("waiting_to_dial: {:?}", waiting_to_dial.len());

                        let address = pending_membership
                            .peer_address(&peer_id)
                            .expect("Peer should exist");

                        debug!("Dialing peer: {:?} {:?}", peer_id, address);

                        let opts = DialOpts::peer_id(peer_id)
                            .condition(PeerCondition::NotDialing)
                            .addresses(vec![address.clone()])
                            .build();

                        debug!("Dialing peer: {:?}", peer_id);
                        Poll::Ready(ToSwarm::Dial { opts })
                    }
                    None => {
                        let all_new_peer_ids = pending_membership.all_peer_ids_ref();

                        let mut connected_peers = HashSet::new();
                        for peer_id in &all_new_peer_ids {
                            if self.all_connections.is_peer_connected(peer_id) {
                                connected_peers.insert(*peer_id);
                            }
                        }
                        let all_connected = connected_peers.len() == all_new_peer_ids.len();
                        if all_connected || *dial_attempts >= MAX_DIAL_ATTEMPT_ROUNDS {
                            let membership_accepted = self
                                .membership_kind
                                .accept(connected_peers.len(), all_new_peer_ids.len());

                            debug!("Membership accepted: {}", membership_accepted);

                            if !membership_accepted {
                                warn!("Failed to establish connections to enough peers.");

                                interval_between_dial_attempts.take();
                                self.state = State::WaitingPeers;
                                return Poll::Ready(ToSwarm::GenerateEvent(Event::NotEnoughPeers));
                            } else {
                                self.state = State::NotifyPeersUpdated;
                                return Poll::Pending;
                            }
                        } else {
                            //Try again few times before notifying the rest of the system about membership update.
                            //Dialing attempt
                            if let Some(interval) = interval_between_dial_attempts {
                                if interval.poll_tick(cx) == Poll::Pending {
                                    return Poll::Pending;
                                } else {
                                    *dial_attempts += 1;
                                    trace!("Next attempt({dial_attempts:?}) to dial failed peers");
                                }
                            } else {
                                let start_at = time::Instant::now() + Duration::from_secs(5);
                                *interval_between_dial_attempts =
                                    Some(time::interval_at(start_at, Duration::from_secs(10)));
                            }
                            waiting_to_dial
                                .extend(all_new_peer_ids.difference(&connected_peers).cloned());
                        }
                        Poll::Pending
                    }
                }
            }
            State::NotifyPeersUpdated => {
                let mut pending = self.memberships.remove_pending();
                let active_peers = match pending.take() {
                    Some(membership) => {
                        let active_peer_ids = membership.active_peer_ids();
                        self.memberships.update(membership);
                        active_peer_ids
                    }
                    None => self.memberships.current().active_peer_ids(),
                };
                self.state = State::WaitingPeers;
                Poll::Ready(ToSwarm::GenerateEvent(Event::PeersUpdated(active_peers)))
            }
        }
    }
}
