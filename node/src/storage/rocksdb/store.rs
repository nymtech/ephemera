use std::sync::Arc;

use crate::block::types::block::Block;
use crate::storage::rocksdb::{block_height_key, block_id_key, last_block_key, signatures_key};
use rocksdb::TransactionDB;

use crate::utilities::crypto::Signature;

pub struct DbStore {
    database: Arc<TransactionDB>,
}

impl DbStore {
    pub fn new(db: Arc<TransactionDB>) -> DbStore {
        DbStore { database: db }
    }

    pub(crate) fn store_block(
        &self,
        block: &Block,
        signatures: Vec<Signature>,
    ) -> anyhow::Result<()> {
        log::debug!("Storing block: {}", block.header);
        let block_id_key = block_id_key(&block.header.id);
        log::trace!("Block id key: {}", block_id_key);

        let signatures_key = signatures_key(&block.header.id);
        log::trace!("Block signatures key: {}", signatures_key);

        let height_key = block_height_key(&block.header.height);

        let tx = self.database.transaction();

        // Check UNIQUE constraints
        let existing_id = tx.get(&block_id_key)?;
        if existing_id.is_some() {
            return Err(anyhow::anyhow!("Block already exists"));
        }

        //Store last block id(without prefix!)
        //May want to check that height is incremented by 1
        tx.put(last_block_key(), block.header.id.as_bytes())?;

        // Store block height
        tx.put(height_key.as_bytes(), block.header.id.as_bytes())?;

        // Store block(without signature)
        let block_bytes = serde_json::to_vec::<Block>(block)?;
        tx.put(block_id_key.as_bytes(), block_bytes)?;

        // Store block signatures
        let signatures_bytes = serde_json::to_vec(&signatures)?;
        tx.put(signatures_key.as_bytes(), signatures_bytes)?;

        tx.commit().map_err(|e| anyhow::anyhow!(e))
    }
}
