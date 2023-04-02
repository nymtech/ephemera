use std::collections::HashMap;

use crate::block::types::message::EphemeraMessage;
use crate::utilities::hash::HashType;

pub(super) struct MessagePool {
    pending_messages: HashMap<HashType, EphemeraMessage>,
}

impl MessagePool {
    pub(super) fn new() -> Self {
        Self {
            pending_messages: HashMap::with_capacity(10000),
        }
    }

    pub(crate) fn contains(&self, hash: &HashType) -> bool {
        self.pending_messages.contains_key(hash)
    }

    pub(super) fn add_message(&mut self, msg: EphemeraMessage) -> anyhow::Result<()> {
        let msg_hash = msg.hash_with_default_hasher()?;

        log::debug!("Adding message to pool: {}", msg_hash);

        self.pending_messages.insert(msg_hash, msg);

        log::debug!("Message pool size: {:?}", self.pending_messages.len());
        Ok(())
    }

    pub(super) fn remove_messages(&mut self, messages: &[EphemeraMessage]) -> anyhow::Result<()> {
        log::trace!("Removing messages from pool: {:?}", messages);
        log::trace!(
            "Mempool size before removing messages {}",
            self.pending_messages.len()
        );
        for msg in messages {
            let hash = msg.hash_with_default_hasher()?;
            if self.pending_messages.remove(&hash).is_none() {
                log::warn!("Message not found in pool: {:?}", msg);
            }
        }
        log::trace!(
            "Mempool size after removing messages {}",
            self.pending_messages.len()
        );
        Ok(())
    }

    /// Returns a `Vec` of all `EphemeraMessage`s in the message pool.
    /// The message pool is not cleared.
    pub(super) fn get_messages(&self) -> Vec<EphemeraMessage> {
        self.pending_messages.values().cloned().collect()
    }
}

#[cfg(test)]
mod test {
    use crate::block::message_pool::MessagePool;
    use crate::block::types::message::EphemeraMessage;
    use crate::crypto::{EphemeraKeypair, Keypair};

    #[test]
    fn test_add_remove() {
        let message =
            EphemeraMessage::signed("label1".to_string(), vec![0], &Keypair::generate(None))
                .unwrap();

        let mut pool = MessagePool::new();
        pool.add_message(message.clone()).unwrap();
        pool.remove_messages(&[message]).unwrap();

        assert_eq!(pool.get_messages().len(), 0);
    }
}
