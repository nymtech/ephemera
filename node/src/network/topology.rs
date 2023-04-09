use lru::LruCache;
use std::collections::HashSet;
use std::num::NonZeroUsize;

use crate::network::peer::PeerId;
use crate::utilities::hash::HashType;

pub(crate) struct BroadcastTopology {
    /// The current id of the topology. Incremented every time a new snapshot is added.
    pub(crate) current_id: u64,
    /// A cache of the topology snapshots.
    pub(crate) topology_snapshots: LruCache<u64, HashSet<PeerId>>,
    /// A cache of the block topologies.
    pub(crate) block_topologies: LruCache<HashType, u64>,
}

impl BroadcastTopology {
    pub(crate) fn new() -> BroadcastTopology {
        let mut topology_snapshots = LruCache::new(NonZeroUsize::new(100).unwrap());
        topology_snapshots.put(0, HashSet::new());
        BroadcastTopology {
            current_id: 0,
            topology_snapshots,
            block_topologies: LruCache::new(NonZeroUsize::new(1000).unwrap()),
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

    pub(crate) fn check_broadcast_message(
        &mut self,
        hash: HashType,
        block_creator: &PeerId,
        message_ender: &PeerId,
    ) -> bool {
        //We see this block first time
        if !self.block_topologies.contains(&hash) {
            //This can happen at startup for example when node is not ready yet(caught up with the network)
            if self.is_empty() {
                log::warn!(
                    "Received new block {:?} but current topology is empty, rejecting the block",
                    hash
                );
                return false;
            }
        }

        //Make sure that the sender peer_id and block peer_id are part of the block initial topology
        //1. If the block is new, the topology is the current one
        //2. If the block is old, the topology is the one that was used when the block was created

        //It's needed to make sure that
        //1. The peer is authenticated(part of the network)
        //2. Block processing is consistent regarding the membership across rounds

        let topology_id = self
            .block_topologies
            .get(&hash)
            .unwrap_or(&self.current_id)
            .clone();

        //Node is excluded from topology for some reason(for example health checks failed)
        if !self.is_member(self.current_id, message_ender) {
            log::warn!(
                "Received new block {} but sender {} is not part of the current topology",
                hash,
                message_ender
            );
            return false;
        }

        //Node is excluded from topology for some reason(for example health checks failed)
        if !self.is_member(self.current_id, block_creator) {
            log::warn!(
                "Received new block {} but sender {} is not part of the current topology",
                hash,
                message_ender
            );
            return false;
        }

        self.block_topologies.put(hash, topology_id);

        true
    }
}
