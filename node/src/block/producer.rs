use crate::block::message_pool::MessagePool;
use crate::block::types::block::{Block, BlockHeader, RawBlock, RawBlockHeader};
use crate::block::types::message::EphemeraMessage;
use crate::utilities::crypto::PeerId;
use crate::utilities::merkle::Merkle;

pub(super) struct BlockProducer {
    message_pool: MessagePool,
    pub(crate) peer_id: PeerId,
}

impl BlockProducer {
    pub(super) fn new(peer_id: PeerId) -> Self {
        let message_pool = MessagePool::new();
        Self {
            message_pool,
            peer_id,
        }
    }

    pub(super) fn produce_block(
        &mut self,
        previous_block_header: BlockHeader,
    ) -> anyhow::Result<Block> {
        let pending_messages = self.message_pool.get_messages();
        log::debug!("Adding {:?} messages in new block", pending_messages.len());
        log::trace!("Pending messages for new block: {:?}", pending_messages);

        let block = self.new_block(previous_block_header, pending_messages)?;
        log::info!("New block: {}", block);
        Ok(block)
    }

    fn new_block(
        &self,
        prev_block_header: BlockHeader,
        mut messages: Vec<EphemeraMessage>,
    ) -> anyhow::Result<Block> {
        messages.sort_by(|a, b| a.id.cmp(&b.id));

        let height = prev_block_header.height + 1;

        let header = RawBlockHeader::new(self.peer_id, height);
        let raw_block = RawBlock::new(header, messages);

        let block_hash = Merkle::calculate_root_hash(&raw_block)?;
        let block = Block::new(raw_block, block_hash);
        Ok(block)
    }

    pub(super) fn message_pool_mut(&mut self) -> &mut MessagePool {
        &mut self.message_pool
    }
}
