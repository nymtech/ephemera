use std::sync::Arc;

use rocksdb::TransactionDB;

use crate::block::{Block, RawBlock};
use crate::database::rocksdb::{LAST_BLOCK_KEY, PREFIX_BLOCK_ID, PREFIX_LABEL, PREFIX_SIGNATURES};
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
        let block_id_key = format!("{}:{}", PREFIX_BLOCK_ID, block.header.id.clone());
        log::trace!("Block id key: {}", block_id_key);
        let block_label_key = format!("{}:{}", PREFIX_LABEL, block.header.label.clone());
        log::trace!("Block label key: {}", block_label_key);
        let signatures_key = format!(
            "{}:{}:{}",
            PREFIX_SIGNATURES,
            PREFIX_BLOCK_ID,
            block.header.label.clone()
        );
        log::trace!("Block signatures key: {}", signatures_key);

        let tx = self.database.transaction();

        // Block id UNIQUE constraint check
        let existing_id = tx.get(&block_id_key)?;
        if existing_id.is_some() {
            return Err(anyhow::anyhow!("Block already exists"));
        }

        // Block label UNIQUE constraint check
        let existing_label = tx.get(block_label_key.as_bytes())?;
        if existing_label.is_some() {
            return Err(anyhow::anyhow!("Block already exists"));
        }

        //Store block label(without prefix!)
        tx.put(block_label_key.as_bytes(), block.header.id.as_bytes())?;

        // Store last block id(without prefix!)
        tx.put(LAST_BLOCK_KEY, block.header.id.as_bytes())?;

        // Store block(without signature)
        let block_bytes = serde_json::to_vec::<RawBlock>(block.into())?;
        tx.put(block_id_key.as_bytes(), block_bytes)?;

        // Store block signatures
        let signatures_bytes = serde_json::to_vec(&signatures)?;
        tx.put(signatures_key.as_bytes(), signatures_bytes)?;

        tx.commit().map_err(|e| anyhow::anyhow!(e))
    }
}
