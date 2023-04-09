use std::pin::Pin;
use std::task::Poll::Pending;
use std::{task, time};

use futures::FutureExt;
use futures::Stream;
use futures_timer::Delay;
use lru::LruCache;
use thiserror::Error;

use crate::block::message_pool::MessagePool;
use crate::block::producer::BlockProducer;
use crate::block::types::block::Block;
use crate::block::types::message::EphemeraMessage;
use crate::broadcast::signing::BlockSigner;
use crate::config::BlockConfig;
use crate::network::peer::{PeerId, ToPeerId};
use crate::utilities::crypto::Certificate;
use crate::utilities::hash::HashType;

pub(crate) type Result<T> = std::result::Result<T, BlockManagerError>;

#[derive(Error, Debug)]
pub(crate) enum BlockManagerError {
    #[error("Message is already in pool: {0}")]
    DuplicateMessage(String),
    #[error("{0}")]
    Internal(#[from] anyhow::Error),
}

pub(crate) struct BlockManager {
    pub(super) config: BlockConfig,
    /// All blocks what we received from the network or created by us
    pub(super) last_blocks: LruCache<HashType, Block>,
    /// Last block that we created.
    /// It's not Option because we always have genesis block
    pub(super) last_proposed_block: Block,
    /// Last block that we accepted
    /// It's not Option because we always have genesis block
    pub(super) last_committed_block: Block,
    /// Block producer. Simple helper that creates blocks
    pub(super) block_producer: BlockProducer,
    /// Message pool. Contains all messages that we received from the network and not included in any block yet
    pub(super) message_pool: MessagePool,
    /// Delay between block creation attempts.
    pub(super) delay: Delay,
    /// Signs and verifies blocks
    pub(super) block_signer: BlockSigner,
}

impl BlockManager {
    pub(crate) async fn on_new_message(&mut self, msg: EphemeraMessage) -> Result<()> {
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
    ) -> anyhow::Result<()> {
        //Block signer should be also its sender
        let signer_peer_id = certificate.public_key.peer_id();
        if *sender != signer_peer_id {
            anyhow::bail!(format!(
                "Block creator and signer are different: creator = {sender} != {signer_peer_id}",
            ));
        }

        //Reject blocks with invalid hash
        let hash = block.hash_with_default_hasher()?;
        if block.header.hash != hash {
            anyhow::bail!(format!(
                "Block hash mismatch: {:?} != {hash:?}",
                block.header.hash,
            ));
        }

        //Verify that block signature is valid
        if self.block_signer.verify_block(block, certificate).is_err() {
            anyhow::bail!(format!(
                "Block signature verification failed: {:?}",
                block.header.hash
            ));
        }

        if self.last_blocks.contains(&hash) {
            log::trace!("Block already received: {}", block.header.hash);
            return Ok(());
        }

        log::debug!("Block received: {}", block.header.hash);
        self.last_blocks.put(hash, block.to_owned());
        Ok(())
    }

    pub(crate) fn sign_block(&mut self, block: &Block) -> anyhow::Result<Certificate> {
        let hash = block.hash_with_default_hasher()?;
        let certificate = self.block_signer.sign_block(block, &hash)?;
        Ok(certificate)
    }

    /// After a block gets committed, clear up mempool from its messages
    pub(crate) fn on_block_committed(&mut self, block: &Block) -> anyhow::Result<()> {
        let hash = &block.header.hash;
        if let Some(block) = self.last_blocks.get(hash) {
            if block.header.creator != self.block_producer.peer_id {
                log::debug!("Not locally created block {hash:?}, not cleaning mempool");
                return Ok(());
            }

            //FIXME...: handle gaps in block heights
            if self.last_committed_block.get_hash() != *hash {
                log::warn!(
                    "Received committed block {hash} after we created next block, ignoring..."
                );
                return Ok(());
            }

            self.message_pool.remove_messages(&block.messages)?;
            log::debug!("Mempool cleaned up from block: {:?} messages", block)
        } else {
            //TODO: something to think about
            log::error!("Block not found: {}", block);
        }
        Ok(())
    }

    pub(crate) fn get_block_by_hash(&mut self, block_id: &HashType) -> Option<Block> {
        self.last_blocks.get(block_id).cloned()
    }

    pub(crate) fn get_block_signatures(&mut self, hash: &HashType) -> Option<Vec<Certificate>> {
        self.block_signer.get_block_signatures(hash)
    }

    pub(crate) fn accept_last_proposed_block(&mut self) {
        self.last_committed_block = self.last_proposed_block.clone();
    }
}

//Produces blocks at a predefined interval.
//If blocks will be actually broadcast is a different question and depends on the application.
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

        match self.delay.poll_unpin(cx) {
            task::Poll::Ready(_) => {
                //FIXME...: this all is very loose now, no checking of caps etc
                let previous_block_header = self.last_committed_block.header.clone();
                let new_height = previous_block_header.height + 1;
                let pending_messages = self.message_pool.get_messages();

                let result = match self
                    .block_producer
                    .produce_block(new_height, pending_messages)
                {
                    Ok(block) => {
                        let hash = block
                            .hash_with_default_hasher()
                            .expect("Failed to hash block");

                        log::debug!("Produced block: {:?}", hash);
                        self.last_proposed_block = block.clone();
                        self.last_blocks.put(hash, block.clone());

                        let certificate = self
                            .block_signer
                            .sign_block(&block, &hash)
                            .expect("Failed to sign block");

                        task::Poll::Ready(Some((block, certificate)))
                    }
                    Err(err) => {
                        log::error!("Error producing block: {:?}", err);
                        Pending
                    }
                };

                let interval = self.config.creation_interval_sec;
                self.delay.reset(time::Duration::from_secs(interval));
                result
            }
            Pending => Pending,
        }
    }
}

#[cfg(test)]
mod test {
    use std::num::NonZeroUsize;
    use std::sync::Arc;
    use std::time::Duration;

    use assert_matches::assert_matches;

    use crate::crypto::{EphemeraKeypair, Keypair};
    use crate::ephemera_api::RawApiEphemeraMessage;
    use crate::network::peer::{PeerId, ToPeerId};

    use super::*;

    #[tokio::test]
    async fn test_add_message() {
        let keypair: Arc<Keypair> = Keypair::generate(None).into();
        let peer_id = keypair.public_key().peer_id();
        let mut manager = block_manager(keypair.clone(), peer_id);

        let message = RawApiEphemeraMessage::new("test".to_string(), vec![1, 2, 3]);
        let signed_message = message.sign(&keypair).expect("Failed to sign message");
        let signed_message: EphemeraMessage = signed_message.into();

        let hash = signed_message.hash_with_default_hasher().unwrap();

        manager.on_new_message(signed_message).await.unwrap();

        assert!(manager.message_pool.contains(&hash));
    }

    #[tokio::test]
    async fn test_add_duplicate_message() {
        let keypair: Arc<Keypair> = Keypair::generate(None).into();
        let peer_id = keypair.public_key().peer_id();
        let mut manager = block_manager(keypair.clone(), peer_id);

        let message = RawApiEphemeraMessage::new("test".to_string(), vec![1, 2, 3]);
        let signed_message = message.sign(&keypair).expect("Failed to sign message");
        let signed_message: EphemeraMessage = signed_message.into();

        manager
            .on_new_message(signed_message.clone())
            .await
            .unwrap();
        assert_matches!(
            manager.on_new_message(signed_message).await,
            Err(BlockManagerError::DuplicateMessage(_))
        );
    }

    #[test]
    fn test_accept_valid_block() {
        let keypair: Arc<Keypair> = Keypair::generate(None).into();
        let peer_id = keypair.public_key().peer_id();
        let mut manager = block_manager(keypair, peer_id);
        let block = block();
        let certificate = manager.sign_block(&block).unwrap();

        let result = manager.on_block(&peer_id, &block, &certificate);

        assert!(result.is_ok());
    }

    #[test]
    fn test_reject_invalid_sender() {
        let keypair: Arc<Keypair> = Keypair::generate(None).into();
        let peer_id = keypair.public_key().peer_id();
        let mut manager = block_manager(keypair, peer_id);
        let block = block();
        let certificate = manager.sign_block(&block).unwrap();

        let invalid_peer_id = PeerId::from_public_key(&Keypair::generate(None).public_key());
        let result = manager.on_block(&invalid_peer_id, &block, &certificate);

        assert!(result.is_err());
    }

    #[test]
    fn test_reject_invalid_hash() {
        let keypair: Arc<Keypair> = Keypair::generate(None).into();
        let peer_id = keypair.public_key().peer_id();
        let mut manager = block_manager(keypair, peer_id);
        let mut block = block();
        let certificate = manager.sign_block(&block).unwrap();

        block.header.hash = HashType::new([0; 32]);
        let result = manager.on_block(&peer_id, &block, &certificate);

        assert!(result.is_err());
    }

    #[test]
    fn test_reject_invalid_signature() {
        let keypair: Arc<Keypair> = Keypair::generate(None).into();
        let peer_id = keypair.public_key().peer_id();
        let mut manager = block_manager(keypair, peer_id);

        let correct_block = block();
        let fake_block = block();

        let fake_certificate = manager.sign_block(&fake_block).unwrap();
        let correct_certificate = manager.sign_block(&correct_block).unwrap();

        let result = manager.on_block(&peer_id, &correct_block, &fake_certificate);
        assert!(result.is_err());

        let result = manager.on_block(&peer_id, &fake_block, &correct_certificate);
        assert!(result.is_err());
    }

    #[test]
    fn test_on_block_committed() {
        //TODO: how to handle non-termination?
    }

    fn block_manager(keypair: Arc<Keypair>, peer_id: PeerId) -> BlockManager {
        let genesis_block = Block::new_genesis_block(peer_id.clone());
        BlockManager {
            config: BlockConfig {
                producer: true,
                creation_interval_sec: 0,
            },
            last_blocks: LruCache::new(NonZeroUsize::new(10).unwrap()),
            last_proposed_block: genesis_block.clone(),
            last_committed_block: genesis_block.clone(),
            block_producer: BlockProducer::new(peer_id.clone()),
            message_pool: MessagePool::new(),
            delay: Delay::new(Duration::from_secs(0)),
            block_signer: BlockSigner::new(keypair.clone()),
        }
    }

    fn block() -> Block {
        let keypair: Arc<Keypair> = Keypair::generate(None).into();
        let peer_id = keypair.public_key().peer_id();
        let mut producer = BlockProducer::new(peer_id.clone());
        producer.produce_block(1, vec![]).unwrap()
    }
}
