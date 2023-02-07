use std::sync::Arc;

use rocksdb::TransactionDB;

use crate::block::Block;
use crate::database::rocksdb::LAST_BLOCK_KEY;

pub struct DbStore {
    database: Arc<TransactionDB>,
}

impl DbStore {
    pub fn new(db: Arc<TransactionDB>) -> anyhow::Result<DbStore> {
        Ok(DbStore { database: db })
    }

    pub(crate) fn store_block(&mut self, block: &Block) -> anyhow::Result<()> {
        log::debug!("Storing block: {}", block.header);

        let tx = self.database.transaction();

        let option = tx.get(block.header.id.as_bytes())?;
        if option.is_some() {
            return Err(anyhow::anyhow!("Block already exists"));
        }

        let id = block.header.id.clone();
        let block_bytes = serde_json::to_vec(&block).unwrap();
        tx.put(id.as_bytes(), block_bytes)?;

        tx.put(LAST_BLOCK_KEY, block.header.id.as_bytes())?;

        tx.commit().map_err(|e| anyhow::anyhow!(e))
    }
}
