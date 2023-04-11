use futures_timer::Delay;
use std::sync::Arc;
use std::time;

use crate::block::manager::{BlockChainState, BlockManager};
use crate::block::message_pool::MessagePool;
use crate::block::producer::BlockProducer;
use crate::block::types::block::Block;
use crate::broadcast::signing::BlockSigner;
use crate::config::BlockConfig;
use crate::crypto::Keypair;
use crate::network::peer::ToPeerId;
use crate::storage::EphemeraDatabase;

pub(crate) struct BlockManagerBuilder {
    config: BlockConfig,
    block_producer: BlockProducer,
    delay: Delay,
    keypair: Arc<Keypair>,
}

impl BlockManagerBuilder {
    pub(crate) fn new(config: BlockConfig, keypair: Arc<Keypair>) -> Self {
        let block_producer = BlockProducer::new(keypair.peer_id());
        let delay = Delay::new(time::Duration::from_secs(config.creation_interval_sec));
        Self {
            config,
            block_producer,
            delay,
            keypair,
        }
    }

    pub(crate) fn build<D: EphemeraDatabase + ?Sized>(
        self,
        storage: &mut D,
    ) -> anyhow::Result<BlockManager> {
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

        let last_created_block = most_recent_block.expect("Block should be present");

        let block_signer = BlockSigner::new(self.keypair.clone());
        let message_pool = MessagePool::new();
        let block_chain_state = BlockChainState::new(last_created_block);

        Ok(BlockManager {
            config: self.config,
            block_producer: self.block_producer,
            block_signer,
            delay: self.delay,
            message_pool,
            block_chain_state,
        })
    }
}
