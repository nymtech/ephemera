use std::fmt::Debug;
use std::sync::Arc;

use rocksdb::TransactionDB;

use crate::block::Block;
use crate::database::rocksdb::LAST_BLOCK_KEY;

pub struct DbQuery {
    database: Arc<TransactionDB>,
}

impl DbQuery {
    pub fn new(db: Arc<TransactionDB>) -> anyhow::Result<DbQuery> {
        Ok(DbQuery { database: db })
    }

    pub(crate) fn get_block_by_id<I: AsRef<[u8]> + Debug>(
        &self,
        block_id: I,
    ) -> anyhow::Result<Option<Block>> {
        log::trace!("Getting block by id: {:?}", block_id);

        let block = if let Some(block) = self.database.get(block_id)? {
            let block: Block = serde_json::from_slice(&block)?;
            log::debug!("Found block: {}", block.header);
            Some(block)
        } else {
            log::debug!("Didn't find block");
            None
        };
        Ok(block)
    }

    pub(crate) fn get_last_block(&self) -> anyhow::Result<Option<Block>> {
        log::debug!("Getting last block");

        if let Some(block_id) = self.database.get(LAST_BLOCK_KEY)? {
            self.get_block_by_id(block_id)
        } else {
            log::debug!("Unable to get LAST_BLOCK_KEY");
            Ok(None)
        }
    }
}
