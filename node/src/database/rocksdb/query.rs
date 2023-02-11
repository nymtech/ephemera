use std::sync::Arc;

use rocksdb::TransactionDB;

use crate::block::{Block, RawBlock};
use crate::database::rocksdb::{block_id_key, label_key, last_block_key, signatures_key};
use crate::utilities::crypto::Signature;

pub struct DbQuery {
    database: Arc<TransactionDB>,
}

impl DbQuery {
    pub fn new(db: Arc<TransactionDB>) -> anyhow::Result<DbQuery> {
        Ok(DbQuery { database: db })
    }

    pub(crate) fn get_block_by_id(&self, block_id: String) -> anyhow::Result<Option<Block>> {
        log::trace!("Getting block by id: {:?}", block_id);

        let block_id_key = block_id_key(&block_id);

        let block = if let Some(block) = self.database.get(block_id_key)? {
            let block = serde_json::from_slice::<RawBlock>(&block)?;
            log::debug!("Found block: {}", block.header);
            Some(block.into())
        } else {
            log::debug!("Didn't find block");
            None
        };
        Ok(block)
    }

    pub(crate) fn get_last_block(&self) -> anyhow::Result<Option<Block>> {
        log::debug!("Getting last block");

        if let Some(block_id) = self.database.get(last_block_key())? {
            self.get_block_by_id(String::from_utf8(block_id)?)
        } else {
            log::debug!("Unable to get last block");
            Ok(None)
        }
    }

    pub(crate) fn get_block_by_label(&self, label: &str) -> anyhow::Result<Option<Block>> {
        log::debug!("Getting block by label: {}", label);

        let block_label = label_key(label);

        if let Some(block_id) = self.database.get(block_label)? {
            self.get_block_by_id(String::from_utf8(block_id)?)
        } else {
            log::debug!("Unable to get block by label {label}");
            Ok(None)
        }
    }

    pub(crate) fn get_block_signatures(
        &self,
        block_id: String,
    ) -> anyhow::Result<Option<Vec<Signature>>> {
        log::debug!("Getting block signatures: {}", block_id);

        let signatures_key = signatures_key(&block_id);

        if let Some(signatures) = self.database.get(signatures_key)? {
            let signatures: Vec<Signature> = serde_json::from_slice(&signatures)?;
            log::debug!("Found signatures: {:?}", signatures);
            Ok(Some(signatures))
        } else {
            log::debug!("Didn't find signatures");
            Ok(None)
        }
    }
}
