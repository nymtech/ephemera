use std::collections::HashMap;

use crate::block::manager::BlockManagerError;
use crate::block::types::message::EphemeraMessage;

pub(super) struct MessagePool {
    pending_messages: HashMap<String, EphemeraMessage>,
}

impl MessagePool {
    pub(super) fn new() -> Self {
        Self {
            pending_messages: HashMap::with_capacity(1000),
        }
    }

    pub(super) fn add_message(&mut self, msg: EphemeraMessage) -> Result<(), BlockManagerError> {
        if self.pending_messages.contains_key(&msg.id) {
            return Err(BlockManagerError::DuplicateMessage(msg.id));
        }
        let msg_id = msg.id.clone();
        log::debug!("Adding message to pool: {}", msg_id);
        self.pending_messages.insert(msg.id.clone(), msg);

        log::debug!("Message pool size: {:?}", self.pending_messages.len());
        Ok(())
    }

    pub(super) fn remove_messages(&mut self, messages: &Vec<EphemeraMessage>) {
        log::trace!("Removing messages from pool: {:?}", messages);
        log::trace!(
            "Mempool size before removing messages {}",
            self.pending_messages.len()
        );
        for msg in messages {
            if self.pending_messages.remove(&msg.id).is_none() {
                log::warn!("Message not found in pool: {:?}", msg);
            }
        }
        log::trace!(
            "Mempool size after removing messages {}",
            self.pending_messages.len()
        );
    }

    /// Returns a `Vec` of all `EphemeraMessage`s in the message pool.
    /// The message pool is not cleared.
    pub(super) fn get_messages(&self) -> Vec<EphemeraMessage> {
        self.pending_messages.values().cloned().collect()
    }
}
