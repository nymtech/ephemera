use crate::{
    block::{
        types::block::{Block, RawBlock, RawBlockHeader},
        types::message::EphemeraMessage,
    },
    network::peer::PeerId,
};

pub(super) struct BlockProducer {
    pub(crate) peer_id: PeerId,
}

impl BlockProducer {
    pub(super) fn new(peer_id: PeerId) -> Self {
        Self { peer_id }
    }

    pub(super) fn produce_block(
        &mut self,
        height: u64,
        pending_messages: Vec<EphemeraMessage>,
    ) -> anyhow::Result<Block> {
        log::debug!("Adding {:?} messages in new block", pending_messages.len());
        log::trace!("Pending messages for new block: {:?}", pending_messages);

        let block = self.new_block(height, pending_messages)?;

        log::debug!("New block: {}", block);
        Ok(block)
    }

    fn new_block(&self, height: u64, mut messages: Vec<EphemeraMessage>) -> anyhow::Result<Block> {
        messages.sort();

        let raw_header = RawBlockHeader::new(self.peer_id, height);
        let raw_block = RawBlock::new(raw_header, messages);

        let block_hash = raw_block.hash_with_default_hasher()?;

        let block = Block::new(raw_block, block_hash);
        Ok(block)
    }
}

#[cfg(test)]
mod test {
    use crate::crypto::Keypair;
    use crate::utilities::EphemeraKeypair;

    use super::*;

    #[test]
    fn test_produce_block() {
        let peer_id = PeerId::random();

        let mut block_producer = BlockProducer::new(peer_id);

        let message1 =
            EphemeraMessage::signed("label1".to_string(), vec![0], &Keypair::generate(None))
                .unwrap();
        let message2 =
            EphemeraMessage::signed("label2".to_string(), vec![1], &Keypair::generate(None))
                .unwrap();
        let messages = vec![message1.clone(), message2.clone()];

        let block = block_producer.produce_block(1, messages).unwrap();

        assert_eq!(block.header.height, 1);
        assert_eq!(block.header.creator, peer_id);
        assert_eq!(block.messages.len(), 2);

        //Nondeterministic because of timestamp
        match message1.cmp(&message2) {
            std::cmp::Ordering::Less => {
                assert_eq!(block.messages[0], message1);
                assert_eq!(block.messages[1], message2);
            }
            std::cmp::Ordering::Greater => {
                assert_eq!(block.messages[0], message2);
                assert_eq!(block.messages[1], message1);
            }
            _ => panic!("Messages are equal"),
        }
    }
}
