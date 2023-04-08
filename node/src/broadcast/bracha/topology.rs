use lru::LruCache;
use std::collections::HashSet;
use std::num::NonZeroUsize;

use crate::network::peer::PeerId;

pub(crate) struct BroadcastTopology {
    /// The current id of the topology. Incremented every time a new snapshot is added.
    pub(crate) current_id: u64,
    /// A cache of the topology snapshots.
    pub(crate) topology_snapshots: LruCache<u64, HashSet<PeerId>>,
}

impl BroadcastTopology {
    pub(crate) fn new() -> BroadcastTopology {
        BroadcastTopology {
            current_id: 0,
            topology_snapshots: LruCache::new(NonZeroUsize::new(1000).unwrap()),
        }
    }

    pub(crate) fn add_snapshot(&mut self, snapshot: Vec<PeerId>) {
        self.current_id += 1;
        self.topology_snapshots
            .put(self.current_id, snapshot.into_iter().collect());
    }

    pub(crate) fn is_member(&mut self, id: u64, peer_id: &PeerId) -> bool {
        self.topology_snapshots
            .get(&id)
            .map(|s| s.contains(peer_id))
            .unwrap_or(false)
    }

    pub(crate) fn is_empty(&mut self) -> bool {
        self.topology_snapshots
            .get(&self.current_id)
            .map(|s| s.is_empty())
            .unwrap_or(true)
    }
}
