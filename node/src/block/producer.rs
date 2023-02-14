use crate::block::message_pool::MessagePool;
use crate::block::{Block, BlockHeader, RawBlock, SignedMessage};
use crate::broadcast::PeerId;
use crate::utilities;
use crate::utilities::time::duration_now;

pub(crate) struct BlockProducer {
    message_pool: MessagePool,
    pub(crate) peer_id: PeerId,
}

impl BlockProducer {
    pub(crate) fn new(peer_id: PeerId) -> Self {
        let message_pool = MessagePool::new();
        Self {
            message_pool,
            peer_id,
        }
    }

    pub(crate) fn produce_block(
        &mut self,
        last_block: Option<Block>,
    ) -> anyhow::Result<Option<Block>> {
        let pending_messages = self.message_pool.get_messages();
        let block = self.new_block(last_block, pending_messages)?;
        Ok(Some(block))
    }

    pub(crate) fn message_pool_mut(&mut self) -> &mut MessagePool {
        &mut self.message_pool
    }

    fn new_block(
        &mut self,
        last_block: Option<Block>,
        mut messages: Vec<SignedMessage>,
    ) -> anyhow::Result<Block> {
        messages.sort_by(|a, b| a.id.cmp(&b.id));

        let mut height = 0;
        if let Some(block) = last_block {
            height = block.header.height + 1;
        }

        let id = utilities::generate_ephemera_id();
        let header = BlockHeader {
            id,
            timestamp: duration_now().as_millis(),
            creator: self.peer_id,
            height,
        };

        let raw_block = RawBlock::new(header, messages);
        let block = Block::new(raw_block);
        Ok(block)
    }
}
