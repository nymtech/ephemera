use std::{
    num::NonZeroUsize,
    pin::Pin,
    task,
    task::Poll::{Pending, Ready},
};

use anyhow::anyhow;
use futures::Stream;

use lru::LruCache;
use thiserror::Error;

use crate::{
    api::application::RemoveMessages,
    block::{
        message_pool::MessagePool,
        producer::BlockProducer,
        types::{block::Block, message::EphemeraMessage},
    },
    broadcast::signing::BlockSigner,
    config::BlockConfig,
    network::peer::{PeerId, ToPeerId},
    utilities::{crypto::Certificate, hash::HashType},
};

pub(crate) type Result<T> = std::result::Result<T, BlockManagerError>;

#[derive(Error, Debug)]
pub(crate) enum BlockManagerError {
    #[error("Message is already in pool: {0}")]
    DuplicateMessage(String),
    //Just a placeholder for now
    #[error("BlockManagerError::GeneralError: {0}")]
    General(#[from] anyhow::Error),
}

/// It helps to use atomic state management for new blocks.
pub(crate) struct BlockChainState {
    pub(crate) last_blocks: LruCache<HashType, Block>,
    /// Last block that we created.
    /// It's not Option because we always have genesis block
    last_produced_block: Option<Block>,
    /// Last block that we accepted
    /// It's not Option because we always have genesis block
    last_committed_block: Block,
}

impl BlockChainState {
    pub(crate) fn new(last_committed_block: Block) -> Self {
        Self {
            //1000 is just a "big enough"
            last_blocks: LruCache::new(NonZeroUsize::new(1000).unwrap()),
            last_produced_block: None,
            last_committed_block,
        }
    }

    fn mark_last_produced_block_as_committed(&mut self) {
        self.last_committed_block = self
            .last_produced_block
            .take()
            .expect("Block should be present");
    }

    fn is_last_produced_block(&self, hash: HashType) -> bool {
        match self.last_produced_block.as_ref() {
            Some(block) => block.get_hash() == hash,
            None => false,
        }
    }

    fn is_last_produced_block_is_pending(&self) -> bool {
        self.last_produced_block.is_some()
    }

    fn next_block_height(&self) -> u64 {
        self.last_committed_block.get_height() + 1
    }

    fn remove_last_produced_block(&mut self) -> Block {
        self.last_produced_block
            .take()
            .expect("Block should be present")
    }
}

pub(crate) struct BlockManager {
    pub(crate) config: BlockConfig,
    /// Block producer. Simple helper that creates blocks
    pub(crate) block_producer: BlockProducer,
    /// Message pool. Contains all messages that we received from the network and not included in any(committed) block yet
    pub(crate) message_pool: MessagePool,
    /// Delay between block creation attempts.
    pub(crate) delay: tokio::time::Interval,
    /// Signs and verifies blocks
    pub(crate) block_signer: BlockSigner,
    /// State management for new blocks
    pub(crate) block_chain_state: BlockChainState,
}

impl BlockManager {
    pub(crate) fn on_new_message(&mut self, msg: EphemeraMessage) -> Result<()> {
        log::debug!("Message received: {:?}", msg);

        let message_hash = msg.hash_with_default_hasher()?;
        if self.message_pool.contains(&message_hash) {
            return Err(BlockManagerError::DuplicateMessage(
                message_hash.to_string(),
            ));
        }

        self.message_pool.add_message(msg)?;
        Ok(())
    }

    pub(crate) fn on_block(
        &mut self,
        sender: &PeerId,
        block: &Block,
        certificate: &Certificate,
    ) -> Result<()> {
        let hash = block.hash_with_default_hasher()?;

        log::debug!(
            "Received block: {:?} from peer {sender:?}",
            block.get_hash()
        );

        //Reject blocks with invalid hash
        if block.header.hash != hash {
            return Err(anyhow!("Block hash is invalid: {} != {hash}", block.header.hash).into());
        }

        //Block signer should be also its sender
        let signer_peer_id = certificate.public_key.peer_id();
        if *sender != signer_peer_id {
            return Err(anyhow!(
                "Block signer is not the block sender: {sender:?} != {signer_peer_id:?}",
            )
            .into());
        }

        //Verify that block signature is valid
        if self.block_signer.verify_block(block, certificate).is_err() {
            return Err(anyhow!("Block signature is invalid: {hash}").into());
        }

        self.block_chain_state
            .last_blocks
            .put(hash, block.to_owned());
        Ok(())
    }

    pub(crate) fn sign_block(&mut self, block: &Block) -> Result<Certificate> {
        let hash = block.hash_with_default_hasher()?;

        log::debug!("Signing block: {hash:?}");

        let certificate = self.block_signer.sign_block(block, &hash)?;

        log::trace!("Block certificate: {certificate:?}",);

        Ok(certificate)
    }

    pub(crate) fn on_application_rejected_block(
        &mut self,
        messages_to_remove: RemoveMessages,
    ) -> Result<()> {
        log::debug!("Application rejected last created block");

        let last_produced_block = self.block_chain_state.remove_last_produced_block();
        match messages_to_remove {
            RemoveMessages::All => {
                let messages = last_produced_block
                    .messages
                    .into_iter()
                    .map(Into::into)
                    .collect::<Vec<_>>();

                log::debug!("Removing block messages from pool: all: {messages:?}",);
                self.message_pool.remove_messages(&messages)?;
            }
            RemoveMessages::Selected(messages) => {
                log::debug!("Removing block messages from pool: selected: {messages:?}",);
                let messages = messages.into_iter().map(Into::into).collect::<Vec<_>>();
                self.message_pool.remove_messages(messages.as_slice())?;
            }
        };
        Ok(())
    }

    /// After a block gets committed, clear up mempool from its messages
    pub(crate) fn on_block_committed(&mut self, block: &Block) -> Result<()> {
        log::debug!("Block committed: {:?}", block);

        let hash = &block.header.hash;

        if !self.block_chain_state.is_last_produced_block(*hash) {
            let last_produced_block = self
                .block_chain_state
                .last_produced_block
                .as_ref()
                .expect("Last produced block should be present");
            return Err(anyhow!(
                "Received committed block {hash} which isn't last produced block {}, this is a bug!",
                last_produced_block.get_hash()).into());
        }

        match self.message_pool.remove_messages(&block.messages) {
            Ok(_) => {
                self.block_chain_state
                    .mark_last_produced_block_as_committed();
            }
            Err(e) => {
                return Err(anyhow!("Failed to remove messages from mempool: {}", e).into());
            }
        }
        Ok(())
    }

    pub(crate) fn get_block_by_hash(&mut self, block_id: &HashType) -> Option<Block> {
        self.block_chain_state.last_blocks.get(block_id).cloned()
    }

    pub(crate) fn get_block_certificates(&mut self, hash: &HashType) -> Option<Vec<Certificate>> {
        self.block_signer.get_block_certificates(hash)
    }
}

//Produces blocks at a predefined interval.
//If blocks will be actually broadcast depends on the application.
impl Stream for BlockManager {
    type Item = (Block, Certificate);

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context,
    ) -> task::Poll<Option<Self::Item>> {
        //Optionally it is possible to turn off block production and let the node behave just as voter.
        //For example for testing purposes.
        if !self.config.producer {
            return Pending;
        }

        match self.delay.poll_tick(cx) {
            Ready(_) => {
                let is_previous_pending =
                    self.block_chain_state.is_last_produced_block_is_pending();
                let pending_messages = if is_previous_pending && self.config.repeat_last_block {
                    let block = self
                        .block_chain_state
                        .last_produced_block
                        .clone()
                        .expect("Block should be present");

                    //Use only previous block messages but create new block with new timestamp
                    log::debug!("Producing block with previous messages");
                    block.messages
                } else {
                    log::debug!("Producing block with new messages");
                    self.message_pool.get_messages()
                };

                let new_height = self.block_chain_state.next_block_height();
                let created_block = self
                    .block_producer
                    .create_block(new_height, pending_messages);

                if let Ok(block) = created_block {
                    log::debug!("Created block: {:?}", block);

                    let hash = block.get_hash();
                    self.block_chain_state.last_produced_block = Some(block.clone());
                    self.block_chain_state.last_blocks.put(hash, block.clone());

                    let certificate = self
                        .block_signer
                        .sign_block(&block, &hash)
                        .expect("Failed to sign block");

                    Ready(Some((block, certificate)))
                } else {
                    log::error!("Error producing block: {:?}", created_block);
                    Pending
                }
            }
            Pending => Pending,
        }
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;
    use std::time::Duration;

    use assert_matches::assert_matches;
    use futures_util::StreamExt;

    use crate::crypto::{EphemeraKeypair, Keypair};
    use crate::ephemera_api::RawApiEphemeraMessage;
    use crate::network::peer::{PeerId, ToPeerId};

    use super::*;

    #[tokio::test]
    async fn test_add_message() {
        let (mut manager, _) = block_manager_with_defaults();

        let signed_message = message("test");
        let hash = signed_message.hash_with_default_hasher().unwrap();

        manager.on_new_message(signed_message).unwrap();

        assert!(manager.message_pool.contains(&hash));
    }

    #[tokio::test]
    async fn test_add_duplicate_message() {
        let (mut manager, _) = block_manager_with_defaults();

        let signed_message = message("test");

        manager.on_new_message(signed_message.clone()).unwrap();

        assert_matches!(
            manager.on_new_message(signed_message),
            Err(BlockManagerError::DuplicateMessage(_))
        );
    }

    #[tokio::test]
    async fn test_accept_valid_block() {
        let (mut manager, peer_id) = block_manager_with_defaults();

        let block = block();
        let certificate = manager.sign_block(&block).unwrap();

        let result = manager.on_block(&peer_id, &block, &certificate);

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_reject_invalid_sender() {
        let (mut manager, _) = block_manager_with_defaults();

        let block = block();
        let certificate = manager.sign_block(&block).unwrap();

        let invalid_peer_id = PeerId::from_public_key(&Keypair::generate(None).public_key());
        let result = manager.on_block(&invalid_peer_id, &block, &certificate);

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_reject_invalid_hash() {
        let (mut manager, peer_id) = block_manager_with_defaults();

        let mut block = block();
        let certificate = manager.sign_block(&block).unwrap();

        block.header.hash = HashType::new([0; 32]);
        let result = manager.on_block(&peer_id, &block, &certificate);

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_reject_invalid_signature() {
        let (mut manager, peer_id) = block_manager_with_defaults();

        let correct_block = block();
        let fake_block = block();

        let fake_certificate = manager.sign_block(&fake_block).unwrap();
        let correct_certificate = manager.sign_block(&correct_block).unwrap();

        let result = manager.on_block(&peer_id, &correct_block, &fake_certificate);
        assert!(result.is_err());

        let result = manager.on_block(&peer_id, &fake_block, &correct_certificate);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_next_block_empty() {
        let (mut manager, _) = block_manager_with_defaults();

        let (block, _) = manager.next().await.unwrap();
        assert_eq!(block.header.height, 1);
        assert!(block.messages.is_empty());
    }

    #[tokio::test]
    async fn test_next_block_with_message() {
        let (mut manager, _) = block_manager_with_defaults();

        let signed_message = message("test");
        manager.on_new_message(signed_message).unwrap();

        match manager.next().await {
            Some((block, _)) => {
                assert_eq!(block.header.height, 1);
                assert_eq!(block.messages.len(), 1);
            }
            None => {
                panic!("No block produced");
            }
        }
    }

    #[tokio::test]
    async fn test_next_block_previous_not_committed_repeat() {
        let (mut manager, _) = block_manager_with_defaults();

        let signed_message = message("test");
        manager.on_new_message(signed_message).unwrap();

        let (block1, _) = manager.next().await.unwrap();

        let signed_message = message("test");
        manager.on_new_message(signed_message).unwrap();

        let (block2, _) = manager.next().await.unwrap();

        assert_eq!(block1.messages.len(), block2.messages.len());
        assert_eq!(block1.header.height, block2.header.height);
    }

    #[tokio::test]
    async fn test_next_block_previous_not_committed_repeat_false() {
        let config = BlockConfig::new(true, 0, false);
        let (mut manager, _) = block_manager_with_config(config);

        let signed_message = message("test");
        manager.on_new_message(signed_message).unwrap();

        let (block1, _) = manager.next().await.unwrap();

        let signed_message = message("test");
        manager.on_new_message(signed_message).unwrap();

        let (block2, _) = manager.next().await.unwrap();

        assert_eq!(block1.messages.len(), 1);
        assert_eq!(block2.messages.len(), 2);
        //We create new block but don't leave gap
        assert_eq!(block1.header.height, block2.header.height);
    }

    #[tokio::test]
    async fn test_on_committed_with_correct_pending_block() {
        let (mut manager, _) = block_manager_with_defaults();

        let signed_message = message("test");
        manager.on_new_message(signed_message).unwrap();

        let (block, _) = manager.next().await.unwrap();

        let result = manager.on_block_committed(&block);

        assert!(result.is_ok());
        assert!(manager.message_pool.get_messages().is_empty());
    }

    #[tokio::test]
    async fn test_on_committed_with_invalid_pending_block() {
        let (mut manager, _) = block_manager_with_defaults();

        let signed_message = message("test");
        manager.on_new_message(signed_message).unwrap();

        let _ = manager.next().await.unwrap();

        //Create invalid block
        let wrong_block = block();

        //This shouldn't remove messages from the pool
        let result = manager.on_block_committed(&wrong_block);
        assert!(result.is_err());
        assert_eq!(manager.message_pool.get_messages().len(), 1);
    }

    #[tokio::test]
    async fn application_rejected_messages_all() {
        let (mut manager, _) = block_manager_with_defaults();

        //Add messages to pool
        let signed_message = message("test");
        manager.on_new_message(signed_message).unwrap();

        let signed_message = message("test");
        manager.on_new_message(signed_message).unwrap();

        //Produce new block
        let _ = manager.next().await.unwrap();

        //Application Rejects the block with ALL messages
        manager
            .on_application_rejected_block(RemoveMessages::All)
            .unwrap();

        assert!(manager.message_pool.get_messages().is_empty());
    }

    #[tokio::test]
    async fn application_rejected_messages_selected() {
        let (mut manager, _) = block_manager_with_defaults();

        //Add messages to pool
        let signed_message1 = message("test");
        manager.on_new_message(signed_message1.clone()).unwrap();

        let signed_message2 = message("test");
        manager.on_new_message(signed_message2.clone()).unwrap();

        //Produce new block
        let _ = manager.next().await.unwrap();

        //Application Rejects the block with ALL messages
        manager
            .on_application_rejected_block(RemoveMessages::Selected(vec![signed_message2.into()]))
            .unwrap();

        assert_eq!(manager.message_pool.get_messages().len(), 1);
        let message = manager
            .message_pool
            .get_messages()
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(message, signed_message1);
    }

    fn block_manager_with_defaults() -> (BlockManager, PeerId) {
        let config = BlockConfig::new(true, 0, true);
        block_manager_with_config(config)
    }

    fn block_manager_with_config(config: BlockConfig) -> (BlockManager, PeerId) {
        let keypair: Arc<Keypair> = Keypair::generate(None).into();
        let peer_id = keypair.public_key().peer_id();
        let genesis_block = Block::new_genesis_block(peer_id);
        let block_chain_state = BlockChainState::new(genesis_block);
        (
            BlockManager {
                config,
                block_producer: BlockProducer::new(peer_id),
                message_pool: MessagePool::new(),
                delay: tokio::time::interval(Duration::from_millis(1)),
                block_signer: BlockSigner::new(keypair),
                block_chain_state,
            },
            peer_id,
        )
    }

    fn block() -> Block {
        let keypair: Arc<Keypair> = Keypair::generate(None).into();
        let peer_id = keypair.public_key().peer_id();
        let mut producer = BlockProducer::new(peer_id);
        producer.create_block(1, vec![]).unwrap()
    }

    fn message(label: &str) -> EphemeraMessage {
        let message1 = RawApiEphemeraMessage::new(label.into(), vec![1, 2, 3]);
        let keypair = Keypair::generate(None);
        let signed_message1 = message1.sign(&keypair).expect("Failed to sign message");
        signed_message1.into()
    }
}
