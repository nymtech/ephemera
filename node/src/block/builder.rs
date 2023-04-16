use log::{debug, info};
use std::{sync::Arc, time, time::Duration};

use crate::block::manager::State;
use crate::{
    block::{
        manager::{BlockChainState, BlockManager},
        message_pool::MessagePool,
        producer::BlockProducer,
        types::block::Block,
    },
    broadcast::signing::BlockSigner,
    config::BlockConfig,
    crypto::Keypair,
    network::peer::ToPeerId,
    storage::EphemeraDatabase,
};

pub(crate) struct BlockManagerBuilder {
    config: BlockConfig,
    block_producer: BlockProducer,
    delay: tokio::time::Interval,
    keypair: Arc<Keypair>,
}

impl BlockManagerBuilder {
    pub(crate) fn new(config: BlockConfig, keypair: Arc<Keypair>) -> Self {
        let block_producer = BlockProducer::new(keypair.peer_id());

        let start_at = tokio::time::Instant::now() + Duration::from_secs(60);
        let delay = tokio::time::interval_at(
            start_at,
            time::Duration::from_secs(config.creation_interval_sec),
        );
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
        let mut most_recent_block = storage.get_last_block()?;
        if most_recent_block.is_none() {
            //Although Ephemera is not a blockchain(chain of historically dependent blocks),
            //it's helpful to have some sort of notion of progress in time. So we use the concept of height.
            //The genesis block helps to define the start of it.

            info!("No last block found in database. Creating genesis block.");

            let genesis_block = Block::new_genesis_block(self.block_producer.peer_id);
            storage.store_block(&genesis_block, vec![])?;
            most_recent_block = Some(genesis_block);
        }

        let last_created_block = most_recent_block.expect("Block should be present");
        debug!("Most recent block: {:?}", last_created_block);

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
            state: State::Paused,
        })
    }
}
