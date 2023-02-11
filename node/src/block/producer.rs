use std::sync::Arc;

use crate::block::callback::{ApplicationInfo, BlockProducerCallback};
use crate::block::manager::BlockManagerError;
use crate::block::message_pool::MessagePool;
use crate::block::{Block, BlockHeader, RawBlock, RawMessage, SignedMessage};
use crate::broadcast::PeerId;
use crate::utilities;
use crate::utilities::crypto::libp2p2_crypto::Libp2pKeypair;
use crate::utilities::crypto::signer::Libp2pSigner;
use crate::utilities::crypto::Signer;
use crate::utilities::time::duration_now;

pub(crate) struct BlockProducer {
    message_pool: MessagePool,
    callback: Box<dyn BlockProducerCallback + Send + 'static>,
    signer: Libp2pSigner,
    pending_messages: Vec<SignedMessage>,
    pub(crate) peer_id: PeerId,
}

impl BlockProducer {
    pub(crate) fn new<C>(callback: C, peer_id: PeerId, key_pair: Arc<Libp2pKeypair>) -> Self
    where
        C: BlockProducerCallback + Send + 'static,
    {
        let message_pool = MessagePool::new();
        let signer = Libp2pSigner::new(key_pair);
        Self {
            message_pool,
            callback: Box::new(callback),
            signer,
            pending_messages: Default::default(),
            peer_id,
        }
    }

    pub(crate) fn produce_block(
        &mut self,
        last_block: Option<Block>,
    ) -> anyhow::Result<Option<Block>> {
        let pending_messages = self.message_pool.get_messages();

        if let Ok(Some(propose_result)) = self.callback.on_proposed_block(&pending_messages) {
            let block = self.new_block(last_block, propose_result.clone())?;

            self.pending_messages = propose_result.accepted_messages.into();

            return Ok(Some(block));
        }
        Err(anyhow::anyhow!("Failed to produce block"))
    }

    pub(crate) fn message_pool_mut(&mut self) -> &mut MessagePool {
        &mut self.message_pool
    }

    pub(crate) fn verify_block(&mut self, block: &Block) -> Result<(), BlockManagerError> {
        let raw_block: RawBlock = (*block).clone().into();
        match self.signer.verify(&raw_block, &block.signature) {
            Ok(true) => Ok(()),
            Ok(false) => Err(BlockManagerError::InvalidBlock(
                "Invalid signature".to_string(),
            )),
            Err(err) => Err(BlockManagerError::InvalidBlock(format!(
                "Invalid signature: {err}",
            ))),
        }
    }

    pub(crate) fn verify_message(&mut self, msg: &SignedMessage) -> Result<(), BlockManagerError> {
        let raw_message: RawMessage = (*msg).clone().into();
        match self.signer.verify(&raw_message, &msg.signature) {
            Ok(true) => Ok(()),
            Ok(false) => Err(BlockManagerError::InvalidMessage(
                "Invalid signature".to_string(),
            )),
            Err(err) => Err(BlockManagerError::InvalidBlock(format!(
                "Invalid signature: {err}",
            ))),
        }
    }

    pub(crate) fn sign_block(&mut self, block: Block) -> anyhow::Result<Block> {
        let raw_block = block.into();
        let signature = self.signer.sign(&raw_block)?;

        let block = Block::new(raw_block, signature);
        Ok(block)
    }

    fn new_block(
        &mut self,
        last_block: Option<Block>,
        application_info: ApplicationInfo,
    ) -> anyhow::Result<Block> {
        let mut sorted_messages = application_info.accepted_messages.to_owned();
        sorted_messages.sort_by(|a, b| a.id.cmp(&b.id));

        let mut height = 0;
        if let Some(block) = last_block {
            height = block.header.height + 1;
        }

        let id = utilities::generate_ephemera_id();
        let header = BlockHeader {
            id: id.clone(),
            timestamp: duration_now().as_millis(),
            creator: self.peer_id,
            height,
            label: application_info.label.unwrap_or(id),
        };

        let raw_block = RawBlock::new(header, sorted_messages);
        let signature = self.signer.sign(&raw_block)?;

        let block = Block::new(raw_block, signature);
        Ok(block)
    }
}
