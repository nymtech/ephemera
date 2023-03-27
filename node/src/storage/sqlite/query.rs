use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Row};

use crate::block::types::block::Block;
use crate::config::DbConfig;
use crate::utilities::crypto::Signature;

pub(crate) struct DbQuery {
    pub(crate) connection: Connection,
}

impl DbQuery {
    pub(crate) fn open(db_conf: DbConfig, flags: OpenFlags) -> anyhow::Result<Self> {
        let connection = Connection::open_with_flags(db_conf.sqlite_path, flags)?;
        let query = Self { connection };
        Ok(query)
    }

    pub(crate) fn get_block_by_id(&self, block_id: String) -> anyhow::Result<Option<Block>> {
        log::trace!("Getting block by id: {}", block_id);

        let mut stmt = self
            .connection
            .prepare_cached("SELECT block FROM blocks WHERE block_id = ?1")?;
        let block = stmt
            .query_row(params![block_id], Self::map_block())
            .optional()?;

        if let Some(block) = &block {
            log::trace!("Found block: {}", block.header);
        } else {
            log::trace!("Block not found: {}", block_id);
        };

        Ok(block)
    }

    pub(crate) fn get_last_block(&self) -> anyhow::Result<Option<Block>> {
        log::trace!("Getting last block");

        let mut stmt = self
            .connection
            .prepare_cached("SELECT block FROM blocks where id = (select max(id) from blocks)")?;

        let block = stmt.query_row(params![], Self::map_block()).optional()?;

        if let Some(block) = &block {
            log::trace!("Found last block: {}", block.header);
        } else {
            log::trace!("Last block not found");
        };

        Ok(block)
    }

    pub(crate) fn get_block_by_height(&self, height: u64) -> anyhow::Result<Option<Block>> {
        log::trace!("Getting block by height: {}", height);

        let mut stmt = self
            .connection
            .prepare_cached("SELECT block FROM blocks WHERE height = ?1")?;
        let block = stmt
            .query_row(params![height], Self::map_block())
            .optional()?;

        if let Some(block) = &block {
            log::trace!("Found block: {}", block.header);
        } else {
            log::trace!("Block not found: {}", height);
        };

        Ok(block)
    }

    pub(crate) fn get_block_signatures(
        &self,
        block_id: String,
    ) -> anyhow::Result<Option<Vec<Signature>>> {
        log::trace!("Getting block signatures by block id {}", block_id);

        let mut stmt = self
            .connection
            .prepare_cached("SELECT signatures FROM signatures where block_id = ?1")?;

        let signatures = stmt
            .query_row(params![block_id], |row| {
                let signatures: Vec<u8> = row.get(0)?;
                let signatures =
                    serde_json::from_slice::<Vec<Signature>>(&signatures).map_err(|e| {
                        log::error!("Error deserializing block: {}", e);
                        rusqlite::Error::InvalidQuery {}
                    })?;
                Ok(signatures)
            })
            .optional()?;

        if signatures.is_some() {
            log::trace!("Found block {} signatures", block_id);
        } else {
            log::trace!("Signatures not found");
        };

        Ok(signatures)
    }

    fn map_block() -> impl FnOnce(&Row) -> Result<Block, rusqlite::Error> {
        |row| {
            let body: Vec<u8> = row.get(0)?;
            let block = serde_json::from_slice::<Block>(&body).map_err(|e| {
                log::error!("Error deserializing block: {}", e);
                rusqlite::Error::InvalidQuery {}
            })?;
            Ok(block)
        }
    }
}
