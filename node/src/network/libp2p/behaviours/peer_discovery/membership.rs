//! In Ephemera, membership of reliable broadcast protocol is decided by peer discovery.
//! Only peers who are returned by [crate::peer_discovery::PeerDiscovery] are allowed to participate.

use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;

use crate::peer::Peer;
use libp2p_identity::PeerId;
use lru::LruCache;

/// Peer discovery returns list of peers. But it is up to the Ephemera user to decide
/// how reliable the list is. For example, it can contain peers who are offline.

/// This enum defines how the actual membership is decided.
#[derive(Debug)]
pub(crate) enum MembershipKind {
    /// Specified threshold of peers(from total provided by [crate::peer_discovery::PeerDiscovery]) need to be available.
    /// Threshold value is defined the ratio of peers that need to be available.
    /// For example, if the threshold is 0.5, then at least 50% of the peers need to be available.
    Threshold(f64),
    /// Membership is defined by peers who are online.
    /// Although it's possible to define it as threshold 0.0, this adds more readability and safety.
    AnyOnline,
    /// It's required that all peers are online.
    /// Although it's possible to define it as threshold 1.0, this adds more readability and safety.
    AllOnline,
}

impl MembershipKind {
    pub(crate) fn accept(&self, membership: &Membership) -> bool {
        let total_number_of_peers = membership.all_members.len();
        let connected_peers = membership.connected_peers.len();
        match self {
            MembershipKind::Threshold(threshold) => {
                let minimum_available_nodes = (total_number_of_peers as f64 * threshold) as usize;
                connected_peers >= minimum_available_nodes
            }
            MembershipKind::AnyOnline => connected_peers > 0,
            MembershipKind::AllOnline => connected_peers == total_number_of_peers,
        }
    }
}

pub(crate) struct Memberships {
    snapshots: LruCache<u64, Membership>,
    current: u64,
    /// This is set when we get new peers set from [crate::peer_discovery::PeerDiscovery]
    /// but haven't yet activated it.
    pending_membership: Option<Membership>,
}

impl Memberships {
    pub(crate) fn new() -> Self {
        let mut snapshots = LruCache::new(NonZeroUsize::new(1000).unwrap());
        snapshots.put(0, Membership::new(Default::default()));
        Self {
            snapshots,
            current: 0,
            pending_membership: None,
        }
    }

    pub(crate) fn current(&mut self) -> &Membership {
        //Unwrap is safe because we always have current membership
        self.snapshots.get(&self.current).unwrap()
    }

    pub(crate) fn update(&mut self, membership: Membership) {
        self.current += 1;
        self.snapshots.put(self.current, membership);
    }

    pub(crate) fn set_pending(&mut self, membership: Membership) {
        self.pending_membership = Some(membership);
    }

    pub(crate) fn remove_pending(&mut self) -> Option<Membership> {
        self.pending_membership.take()
    }

    pub(crate) fn pending(&self) -> Option<&Membership> {
        self.pending_membership.as_ref()
    }

    pub(crate) fn pending_mut(&mut self) -> Option<&mut Membership> {
        self.pending_membership.as_mut()
    }
}

#[derive(Debug)]
pub(crate) struct Membership {
    local_peer_id: PeerId,
    all_members: HashMap<PeerId, Peer>,
    connected_peers: HashSet<PeerId>,
}

impl Membership {
    pub(crate) fn new_with_local(
        all_members: HashMap<PeerId, Peer>,
        local_peer_id: PeerId,
    ) -> Self {
        Self {
            local_peer_id,
            all_members,
            connected_peers: HashSet::new(),
        }
    }

    pub(crate) fn new(all_members: HashMap<PeerId, Peer>) -> Self {
        Self {
            local_peer_id: PeerId::random(),
            all_members,
            connected_peers: HashSet::new(),
        }
    }

    pub(crate) fn includes_local(&self) -> bool {
        self.all_members.contains_key(&self.local_peer_id)
    }

    pub(crate) fn peer_connected(&mut self, peer_id: PeerId) {
        self.connected_peers.insert(peer_id);
    }

    pub(crate) fn peer_disconnected(&mut self, peer_id: &PeerId) {
        self.connected_peers.remove(peer_id);
    }

    pub(crate) fn all_ids_ref(&self) -> HashSet<&PeerId> {
        self.all_members.keys().collect()
    }

    pub(crate) fn connected_peer_ids(&self) -> HashSet<PeerId> {
        self.connected_peers.clone()
    }

    pub(crate) fn connected_peer_ids_with_local(&self) -> HashSet<PeerId> {
        let mut active_peers = self.connected_peers.clone();
        active_peers.insert(self.local_peer_id);
        active_peers
    }

    pub(crate) fn connected_ids_ref(&self) -> HashSet<&PeerId> {
        self.connected_peers.iter().collect()
    }

    pub(crate) fn peer_address(&self, peer_id: &PeerId) -> Option<&libp2p::Multiaddr> {
        self.all_members
            .get(peer_id)
            .map(|peer| peer.address.inner())
    }
}
