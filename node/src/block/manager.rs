use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll::Pending;
use std::{task, time};

use futures::FutureExt;
use futures::Stream;
use futures_timer::Delay;
use lru::LruCache;
use thiserror::Error;

use crate::block::callback::BlockProducerCallback;
use crate::block::producer::BlockProducer;
use crate::block::{Block, SignedMessage};
use crate::broadcast::PeerId;
use crate::config::configuration::{BlockConfig, Configuration};
use crate::utilities::crypto::libp2p2_crypto::Libp2pKeypair;
use crate::utilities::EphemeraId;

#[derive(Error, Debug)]
pub(crate) enum BlockManagerError {
    #[error("Invalid block: '{}'", .0)]
    InvalidBlock(String),
    #[error("Invalid message: '{}'", .0)]
    InvalidMessage(String),
    #[error("Duplicate message: '{}'", .0)]
    DuplicateMessage(String),
}

// !!! BlockManager(and associated structures) are currently accessed by a single thread, guaranteed by tokio::select!
pub(crate) struct BlockManager {
    config: BlockConfig,
    /// All blocks what we received from the network or created by us
    last_blocks: LruCache<String, Block>,
    /// The last block that we created(in case we are the leader)
    last_created_block: Option<Block>,
    block_producer: BlockProducer,
    delay: Delay,
}

impl BlockManager {
    pub(crate) fn new<C>(
        callback: C,
        config: Configuration,
        peer_id: PeerId,
        key_pair: Arc<Libp2pKeypair>,
        last_block_from_db: Option<Block>,
    ) -> Self
    where
        C: BlockProducerCallback + Send + 'static,
    {
        let block_producer = BlockProducer::new(callback, peer_id, key_pair);
        let delay = Delay::new(time::Duration::from_secs(
            config.block_config.block_creation_interval_sec,
        ));

        Self {
            config: config.block_config,
            last_blocks: LruCache::new(NonZeroUsize::new(1000).unwrap()),
            last_created_block: last_block_from_db,
            block_producer,
            delay,
        }
    }

    pub(crate) fn on_block(
        &mut self,
        block: &Block,
    ) -> Result<(), BlockManagerError> {
        log::info!("Block received: {}", block.header.id);
        self.last_blocks.put(block.header.id.clone(), block.to_owned());
        Ok(())
    }

    /// After a block gets committed, clear up mempool from its messages
    pub(crate) fn on_block_committed(
        &mut self,
        block_id: &EphemeraId,
    ) -> Result<(), BlockManagerError> {
        log::debug!("Cleaning mempool from block: {} messages...", block_id);

        if let Some(block) = self.last_blocks.get(&block_id.to_string()) {
            //Usually we clear messages from mempool when a block is committed.
            //But in case of specific configuration where all nodes are leaders, each node clears messages
            //from mempool only if it is it's own block
            if self.config.leader && block.header.creator != self.block_producer.peer_id {
                log::debug!("Not my block {block_id}, not cleaning mempool");
                return Ok(());
            }

            self.block_producer
                .message_pool_mut()
                .remove_messages(&block.signed_messages);
            log::debug!("Mempool cleaned from block {} messages", block_id);
        } else {
            log::warn!("Block not found: {}", block_id);
        }
        Ok(())
    }

    pub(crate) fn get_block_by_id(&mut self, block_id: &EphemeraId) -> Option<Block> {
        self.last_blocks.get(block_id).cloned()
    }

    pub(crate) fn verify_message(&mut self, msg: &SignedMessage) -> Result<(), BlockManagerError> {
        self.block_producer.verify_message(msg)
    }

    pub(crate) async fn new_message(
        &mut self,
        msg: SignedMessage,
    ) -> Result<(), BlockManagerError> {
        self.verify_message(&msg)?;
        self.block_producer.message_pool_mut().add_message(msg)
    }
}

impl Stream for BlockManager {
    type Item = Block;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context,
    ) -> task::Poll<Option<Self::Item>> {
        if !self.config.leader {
            return Pending;
        }
        match self.delay.poll_unpin(cx) {
            task::Poll::Ready(_) => {
                //TODO - also we shouldn't create a new block if last block is not committed but we keep it loose for now
                let last_block = self.last_created_block.clone();

                let result = match self.block_producer.produce_block(last_block) {
                    Ok(Some(block)) => {
                        self.last_created_block = Some(block.clone());
                        self.last_blocks.put(block.header.id.clone(), block.clone());

                        task::Poll::Ready(Some(block))
                    }

                    Ok(None) => Pending,
                    Err(err) => {
                        log::error!("Error producing block: {:?}", err);
                        Pending
                    }
                };
                let interval = self.config.block_creation_interval_sec;
                self.delay.reset(time::Duration::from_secs(interval));
                result
            }
            Pending => Pending,
        }
    }
}
