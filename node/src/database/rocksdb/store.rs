use std::sync::Arc;

use rocksdb::TransactionDB;

use crate::block::Block;
use crate::database::rocksdb::{LAST_BLOCK_KEY, PREFIX_BLOCK_ID, PREFIX_LABEL};

pub struct DbStore {
    database: Arc<TransactionDB>,
}

impl DbStore {
    pub fn new(db: Arc<TransactionDB>) -> anyhow::Result<DbStore> {
        Ok(DbStore { database: db })
    }

    pub(crate) fn store_block(&mut self, block: &Block) -> anyhow::Result<()> {
        log::debug!("Storing block: {}", block.header);
        let block_id = format!("{}{}", PREFIX_BLOCK_ID, block.header.id.clone());
        let block_label = format!("{}{}", PREFIX_LABEL, block.header.label.clone());

        let tx = self.database.transaction();

        // Block id UNIQUE constraint check
        let existing_id = tx.get(&block_id)?;
        if existing_id.is_some() {
            return Err(anyhow::anyhow!("Block already exists"));
        }

        // Block label UNIQUE constraint check
        let existing_label = tx.get(block_label.as_bytes())?;
        if existing_label.is_some() {
            return Err(anyhow::anyhow!("Block already exists"));
        }

        //Store block label(without prefix!)
        tx.put(block_label.as_bytes(), block.header.id.as_bytes())?;

        // Store last block id(without prefix!)
        tx.put(LAST_BLOCK_KEY, block.header.id.as_bytes())?;

        // Store block
        let block_bytes = serde_json::to_vec(&block)?;
        tx.put(block_id.as_bytes(), block_bytes)?;

        tx.commit().map_err(|e| anyhow::anyhow!(e))
    }
}
