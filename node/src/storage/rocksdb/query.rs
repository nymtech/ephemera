use std::sync::Arc;

use crate::block::types::block::Block;
use crate::storage::rocksdb::{block_height_key, block_id_key, last_block_key, signatures_key};
use rocksdb::TransactionDB;

use crate::utilities::crypto::Signature;

pub struct DbQuery {
    database: Arc<TransactionDB>,
}

impl DbQuery {
    pub fn new(db: Arc<TransactionDB>) -> DbQuery {
        DbQuery { database: db }
    }

    pub(crate) fn get_block_by_id(&self, block_id: String) -> anyhow::Result<Option<Block>> {
        log::trace!("Getting block by id: {:?}", block_id);

        let block_id_key = block_id_key(&block_id);

        let block = if let Some(block) = self.database.get(block_id_key)? {
            let block = serde_json::from_slice::<Block>(&block)?;
            log::trace!("Found block: {}", block.header);
            Some(block)
        } else {
            log::trace!("Didn't find block");
            None
        };
        Ok(block)
    }

    pub(crate) fn get_last_block(&self) -> anyhow::Result<Option<Block>> {
        log::trace!("Getting last block");

        if let Some(block_id) = self.database.get(last_block_key())? {
            self.get_block_by_id(String::from_utf8(block_id)?)
        } else {
            log::trace!("Unable to get last block");
            Ok(None)
        }
    }

    pub(crate) fn get_block_by_height(&self, height: u64) -> anyhow::Result<Option<Block>> {
        log::trace!("Getting block by height: {}", height);

        if let Some(block_id) = self.database.get(block_height_key(&height))? {
            self.get_block_by_id(String::from_utf8(block_id)?)
        } else {
            log::trace!("Didn't find block");
            Ok(None)
        }
    }

    pub(crate) fn get_block_signatures(
        &self,
        block_id: String,
    ) -> anyhow::Result<Option<Vec<Signature>>> {
        log::trace!("Getting block signatures: {}", block_id);

        let signatures_key = signatures_key(&block_id);

        if let Some(signatures) = self.database.get(signatures_key)? {
            let signatures: Vec<Signature> = serde_json::from_slice(&signatures)?;
            log::trace!("Found signatures: {:?}", signatures);
            Ok(Some(signatures))
        } else {
            log::trace!("Didn't find signatures");
            Ok(None)
        }
    }
}
