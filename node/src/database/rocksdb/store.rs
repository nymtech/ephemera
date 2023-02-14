use std::sync::Arc;

use rocksdb::TransactionDB;

use crate::block::{Block, RawBlock};
use crate::database::rocksdb::{block_id_key, last_block_key, signatures_key};
use crate::utilities::crypto::Signature;

pub struct DbStore {
    database: Arc<TransactionDB>,
}

impl DbStore {
    pub fn new(db: Arc<TransactionDB>) -> anyhow::Result<DbStore> {
        Ok(DbStore { database: db })
    }

    pub(crate) fn store_block(
        &mut self,
        block: &Block,
        signatures: Vec<Signature>,
    ) -> anyhow::Result<()> {
        log::debug!("Storing block: {}", block.header);
        let block_id_key = block_id_key(&block.header.id);
        log::trace!("Block id key: {}", block_id_key);
        let signatures_key = signatures_key(&block.header.id);
        log::trace!("Block signatures key: {}", signatures_key);

        let tx = self.database.transaction();

        // Block id UNIQUE constraint check
        let existing_id = tx.get(&block_id_key)?;
        if existing_id.is_some() {
            return Err(anyhow::anyhow!("Block already exists"));
        }

        // Store last block id(without prefix!)
        tx.put(last_block_key(), block.header.id.as_bytes())?;

        // Store block(without signature)
        let block_bytes = serde_json::to_vec::<RawBlock>(block.into())?;
        tx.put(block_id_key.as_bytes(), block_bytes)?;

        // Store block signatures
        let signatures_bytes = serde_json::to_vec(&signatures)?;
        tx.put(signatures_key.as_bytes(), signatures_bytes)?;

        tx.commit().map_err(|e| anyhow::anyhow!(e))
    }
}
