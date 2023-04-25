use std::sync::Arc;

use crate::block::types::block::Block;
use crate::storage::rocksdb::{block_hash_key, block_height_key, certificates_key, last_block_key};
use log::trace;
use rocksdb::TransactionDB;

use crate::utilities::crypto::Certificate;

pub struct DbQuery {
    database: Arc<TransactionDB>,
}

impl DbQuery {
    pub fn new(db: Arc<TransactionDB>) -> DbQuery {
        DbQuery { database: db }
    }

    pub(crate) fn get_block_by_hash(&self, block_hash: &str) -> anyhow::Result<Option<Block>> {
        trace!("Getting block by id: {:?}", block_hash);

        let block_id_key = block_hash_key(&block_hash);

        let block = if let Some(block) = self.database.get(block_id_key)? {
            let block = serde_json::from_slice::<Block>(&block)?;
            trace!("Found block: {}", block.header);
            Some(block)
        } else {
            trace!("Didn't find block");
            None
        };
        Ok(block)
    }

    pub(crate) fn get_last_block(&self) -> anyhow::Result<Option<Block>> {
        trace!("Getting last block");

        if let Some(block_id) = self.database.get(last_block_key())? {
            let block_hash = String::from_utf8(block_id)?;
            self.get_block_by_hash(&block_hash)
        } else {
            trace!("Unable to get last block");
            Ok(None)
        }
    }

    pub(crate) fn get_block_by_height(&self, height: u64) -> anyhow::Result<Option<Block>> {
        trace!("Getting block by height: {}", height);

        if let Some(block_id) = self.database.get(block_height_key(&height))? {
            let block_hash = String::from_utf8(block_id)?;
            self.get_block_by_hash(&block_hash)
        } else {
            trace!("Didn't find block");
            Ok(None)
        }
    }

    pub(crate) fn get_block_certificates(
        &self,
        block_hash: &str,
    ) -> anyhow::Result<Option<Vec<Certificate>>> {
        trace!("Getting block signatures: {}", block_hash);

        let certificates_key = certificates_key(&block_hash);

        if let Some(certificates) = self.database.get(certificates_key)? {
            let certificates: Vec<Certificate> = serde_json::from_slice(&certificates)?;
            trace!("Found certificates: {:?}", certificates);
            Ok(Some(certificates))
        } else {
            trace!("Didn't find signatures");
            Ok(None)
        }
    }
}
