use std::sync::Arc;

use rocksdb::TransactionDB;

use crate::block::Block;
use crate::database::rocksdb::{LAST_BLOCK_KEY, PREFIX_BLOCK_ID, PREFIX_LABEL};

pub struct DbQuery {
    database: Arc<TransactionDB>,
}

impl DbQuery {
    pub fn new(db: Arc<TransactionDB>) -> anyhow::Result<DbQuery> {
        Ok(DbQuery { database: db })
    }

    pub(crate) fn get_block_by_id(&self, block_id: String) -> anyhow::Result<Option<Block>> {
        log::trace!("Getting block by id: {:?}", block_id);

        let block_id = format!("{PREFIX_BLOCK_ID}{block_id}");

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
            self.get_block_by_id(String::from_utf8(block_id)?)
        } else {
            log::debug!("Unable to get last block");
            Ok(None)
        }
    }

    pub(crate) fn get_block_by_label(&self, label: &str) -> anyhow::Result<Option<Block>> {
        log::debug!("Getting block by label: {}", label);

        let block_label = format!("{PREFIX_LABEL}{label}");

        if let Some(block_id) = self.database.get(block_label)? {
            self.get_block_by_id(String::from_utf8(block_id)?)
        } else {
            log::debug!("Unable to get block by label {label}");
            Ok(None)
        }
    }
}
