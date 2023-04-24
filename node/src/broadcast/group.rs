use std::collections::HashSet;
use std::num::NonZeroUsize;

use log::warn;
use lru::LruCache;

use crate::peer::PeerId;
use crate::utilities::hash::HashType;

pub(crate) struct BroadcastGroup {
    /// The id of current group. Incremented every time a new snapshot is added.
    pub(crate) current_id: u64,
    /// A cache of the group snapshots.
    pub(crate) snapshots: LruCache<u64, HashSet<PeerId>>,
    /// A cache of the groups for each block.
    pub(crate) block_groups: LruCache<HashType, u64>,
}

impl BroadcastGroup {
    pub(crate) fn new() -> BroadcastGroup {
        let mut snapshots = LruCache::new(NonZeroUsize::new(100).unwrap());
        snapshots.put(0, HashSet::new());
        BroadcastGroup {
            current_id: 0,
            snapshots,
            block_groups: LruCache::new(NonZeroUsize::new(100).unwrap()),
        }
    }

    pub(crate) fn add_snapshot(&mut self, snapshot: HashSet<PeerId>) {
        self.current_id += 1;
        self.snapshots
            .put(self.current_id, snapshot.into_iter().collect());
    }

    pub(crate) fn is_member(&mut self, id: u64, peer_id: &PeerId) -> bool {
        self.snapshots
            .get(&id)
            .map(|s| s.contains(peer_id))
            .unwrap_or(false)
    }

    pub(crate) fn is_empty(&mut self) -> bool {
        self.snapshots
            .get(&self.current_id)
            .map(|s| s.is_empty())
            .unwrap_or(true)
    }

    pub(crate) fn current(&mut self) -> &HashSet<PeerId> {
        self.snapshots
            .get(&self.current_id)
            .expect("Current group should always exist")
    }

    pub(crate) fn check_membership(
        &mut self,
        hash: HashType,
        block_creator: &PeerId,
        message_ender: &PeerId,
    ) -> bool {
        //We see this block first time
        if !self.block_groups.contains(&hash) {
            //This can happen at startup for example when node is not ready yet(caught up with the network)
            if self.is_empty() {
                warn!(
                    "Received new block {:?} but current group is empty, rejecting the block",
                    hash
                );
                return false;
            }
        }

        //Make sure that the sender peer_id and block peer_id are part of the block initial group
        //1. If the block is new, the group is the current one
        //2. If the block is old, the group is the one that was used when the block was created

        //It's needed to make sure that
        //1. The peer is authenticated(part of the network)
        //2. Block processing is consistent regarding the group across rounds

        let membership_id = *self.block_groups.get(&hash).unwrap_or(&self.current_id);

        //Node is excluded from group for some reason(for example health checks failed)
        if !self.is_member(self.current_id, message_ender) {
            warn!(
                "Received new block {} but sender {} is not part of the current group",
                hash, message_ender
            );
            return false;
        }

        //Node is excluded from group for some reason(for example health checks failed)
        if !self.is_member(self.current_id, block_creator) {
            warn!(
                "Received new block {} but sender {} is not part of the current group",
                hash, message_ender
            );
            return false;
        }

        self.block_groups.put(hash, membership_id);

        true
    }
}
