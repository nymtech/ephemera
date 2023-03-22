use std::num::NonZeroUsize;
use std::pin::Pin;
use std::task::Poll::Pending;
use std::{task, time};

use futures::FutureExt;
use futures::Stream;
use futures_timer::Delay;
use lru::LruCache;
use thiserror::Error;

use crate::block::producer::BlockProducer;
use crate::block::types::block::Block;
use crate::block::types::message::EphemeraMessage;
use crate::config::BlockConfig;
use crate::network::PeerId;
use crate::storage::rocksdb::RocksDbStorage;
use crate::storage::EphemeraDatabase;
use crate::utilities::id::EphemeraId;

#[derive(Error, Debug)]
pub(crate) enum BlockManagerError {
    #[error("Duplicate message: '{}'", .0)]
    DuplicateMessage(String),
}

//Using builder so that initial interaction with database is isolated during Ephemera initialization.
pub(crate) struct BlockManagerBuilder {
    config: BlockConfig,
    block_producer: BlockProducer,
    delay: Delay,
}

impl BlockManagerBuilder {
    pub(crate) fn new(config: BlockConfig, peer_id: PeerId) -> Self {
        let block_producer = BlockProducer::new(peer_id);
        let delay = Delay::new(time::Duration::from_secs(config.creation_interval_sec));
        Self {
            config,
            block_producer,
            delay,
        }
    }

    pub(crate) fn build(self, storage: &mut RocksDbStorage) -> anyhow::Result<BlockManager> {
        //Although Ephemera is not a blockchain(chain of historically dependent blocks),
        //it's helpful to have some sort of notion of progress in time. So we use the concept of height.
        //The genesis block helps to define the start of it.
        let mut most_recent_block = storage.get_last_block()?;
        if most_recent_block.is_none() {
            log::info!("No last block found in database. Creating genesis block.");
            let genesis_block = Block::new_genesis_block(self.block_producer.peer_id);
            storage.store_block(&genesis_block, vec![])?;
            most_recent_block = Some(genesis_block);
        }

        let last_blocks =
            LruCache::new(NonZeroUsize::new(1000).ok_or(anyhow::anyhow!("Invalid cache size"))?);
        let last_created_block = most_recent_block.expect("Block should be present");

        Ok(BlockManager {
            config: self.config,
            last_blocks,
            last_created_block,
            block_producer: self.block_producer,
            delay: self.delay,
        })
    }
}

pub(crate) struct BlockManager {
    config: BlockConfig,
    /// All blocks what we received from the network or created by us
    last_blocks: LruCache<String, Block>,
    /// The last block that this node created
    last_created_block: Block,
    block_producer: BlockProducer,
    /// Delay between block creation attempts.
    delay: Delay,
}

impl BlockManager {
    pub(crate) async fn new_message(
        &mut self,
        msg: EphemeraMessage,
    ) -> Result<(), BlockManagerError> {
        //FIXME: remove double indirection
        self.block_producer.message_pool_mut().add_message(msg)
    }

    pub(crate) fn on_block(&mut self, block: &Block) -> Result<(), BlockManagerError> {
        if self.last_blocks.contains(&block.header.id) {
            //TODO: change gossip of blocks to request-response in libp2p
            log::trace!("Block already received: {}", block.header.id);
            return Ok(());
        }
        log::info!("Block received: {}", block.header.id);
        self.last_blocks
            .put(block.header.id.clone(), block.to_owned());
        Ok(())
    }

    /// After a block gets committed, clear up mempool from its messages
    pub(crate) fn on_block_committed(
        &mut self,
        block_id: &EphemeraId,
    ) -> Result<(), BlockManagerError> {
        log::debug!("Cleaning mempool from block: {} messages...", block_id);

        if let Some(block) = self.last_blocks.get(&block_id.to_string()) {
            //TODO: handle gaps in block heights
            if self.last_created_block.get_block_id() != *block_id {
                log::warn!(
                    "Received committed block {block_id} after we created next block, ignoring..."
                );
                return Ok(());
            }

            if block.header.creator != self.block_producer.peer_id {
                log::debug!("Not my block {block_id}, not cleaning mempool");
                return Ok(());
            }

            self.block_producer
                //FIXME: remove double indirection
                .message_pool_mut()
                .remove_messages(&block.messages);

            log::debug!(
                "Mempool cleaned from block {} messages: {}",
                block_id,
                block.messages.len()
            );
        } else {
            //TODO: something to think about
            log::error!("Block not found: {}", block_id);
        }
        Ok(())
    }

    pub(crate) fn get_block_by_id(&mut self, block_id: &EphemeraId) -> Option<Block> {
        self.last_blocks.get(block_id).cloned()
    }
}

//Produces blocks at a predefined interval.
//If blocks actually will be broadcast is a different question and depends on the application.
impl Stream for BlockManager {
    type Item = Block;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context,
    ) -> task::Poll<Option<Self::Item>> {
        //Optionally it is possible to turn off block production and let the node behave just as voter.
        //For example for testing purposes.
        //If this should become common then we should consider using a different approach.
        if !self.config.producer {
            return Pending;
        }

        match self.delay.poll_unpin(cx) {
            task::Poll::Ready(_) => {
                //FIXME: this all is very loose now, no checking of caps etc
                let previous_block_header = self.last_created_block.header.clone();
                let result = match self.block_producer.produce_block(previous_block_header) {
                    Ok(block) => {
                        self.last_created_block = block.clone();
                        self.last_blocks.put(block.header.id.clone(), block.clone());

                        task::Poll::Ready(Some(block))
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
