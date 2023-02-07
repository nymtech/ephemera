use std::collections::HashMap;

use crate::block::manager::BlockManagerError;
use crate::block::SignedMessage;

pub(crate) struct MessagePool {
    pending_messages: HashMap<String, SignedMessage>,
}

impl MessagePool {
    pub(crate) fn new() -> Self {
        Self {
            pending_messages: HashMap::with_capacity(1000),
        }
    }

    pub(crate) fn add_message(&mut self, msg: SignedMessage) -> Result<(), BlockManagerError> {
        if self.pending_messages.contains_key(&msg.id) {
            return Err(BlockManagerError::DuplicateMessage(msg.id));
        }
        log::trace!("Adding message to pool: {:?}", msg);
        self.pending_messages.insert(msg.id.clone(), msg);

        log::trace!("Message pool size: {:?}", self.pending_messages.len());
        Ok(())
    }

    pub(crate) fn remove_messages(&mut self, messages: &Vec<SignedMessage>) {
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

    pub(crate) fn get_messages(&self) -> Vec<SignedMessage> {
        self.pending_messages.values().cloned().collect()
    }
}
