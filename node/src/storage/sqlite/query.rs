use log::{error, trace};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Row};

use crate::block::types::block::Block;
use crate::config::DatabaseConfiguration;
use crate::peer::PeerId;
use crate::utilities::crypto::Certificate;

pub(crate) struct DbQuery {
    pub(crate) connection: Connection,
}

impl DbQuery {
    pub(crate) fn open(db_conf: DatabaseConfiguration, flags: OpenFlags) -> anyhow::Result<Self> {
        let connection = Connection::open_with_flags(db_conf.sqlite_path, flags)?;
        let query = Self { connection };
        Ok(query)
    }

    pub(crate) fn get_block_by_hash(&self, block_hash: &str) -> anyhow::Result<Option<Block>> {
        let mut stmt = self
            .connection
            .prepare_cached("SELECT block FROM blocks WHERE block_hash = ?1")?;
        let block = stmt
            .query_row(params![block_hash], Self::map_block())
            .optional()?;

        if let Some(block) = &block {
            trace!("Found block: {}", block.header);
        } else {
            trace!("Block not found: {}", block_hash);
        };

        Ok(block)
    }

    pub(crate) fn get_last_block(&self) -> anyhow::Result<Option<Block>> {
        let mut stmt = self
            .connection
            .prepare_cached("SELECT block FROM blocks where id = (select max(id) from blocks)")?;

        let block = stmt.query_row(params![], Self::map_block()).optional()?;

        if let Some(block) = &block {
            trace!("Found last block: {}", block.header);
        } else {
            trace!("Last block not found");
        };

        Ok(block)
    }

    pub(crate) fn get_block_by_height(&self, height: u64) -> anyhow::Result<Option<Block>> {
        let mut stmt = self
            .connection
            .prepare_cached("SELECT block FROM blocks WHERE height = ?1")?;
        let block = stmt
            .query_row(params![height], Self::map_block())
            .optional()?;

        if let Some(block) = &block {
            trace!("Found block: {}", block.header);
        } else {
            trace!("Block not found: {}", height);
        };

        Ok(block)
    }

    pub(crate) fn get_block_certificates(
        &self,
        block_hash: &str,
    ) -> anyhow::Result<Option<Vec<Certificate>>> {
        let mut stmt = self
            .connection
            .prepare_cached("SELECT certificates FROM block_certificates where block_hash = ?1")?;

        let signatures = stmt
            .query_row(params![block_hash], |row| {
                let certificates: Vec<u8> = row.get(0)?;
                let certificates = serde_json::from_slice::<Vec<Certificate>>(&certificates)
                    .map_err(|e| {
                        error!("Error deserializing certificates: {}", e);
                        rusqlite::Error::InvalidQuery {}
                    })?;
                Ok(certificates)
            })
            .optional()?;

        if signatures.is_some() {
            trace!("Found block {} certificates", block_hash);
        } else {
            trace!("Certificates not found");
        };

        Ok(signatures)
    }

    pub(crate) fn get_block_broadcast_group(
        &self,
        block_hash: &str,
    ) -> anyhow::Result<Option<Vec<PeerId>>> {
        let mut stmt = self
            .connection
            .prepare_cached("SELECT members FROM block_broadcast_group where block_hash = ?1")?;

        let members = stmt
            .query_row(params![block_hash], |row| {
                let members: Vec<u8> = row.get(0)?;
                let members = serde_json::from_slice::<Vec<PeerId>>(&members).map_err(|e| {
                    error!("Error deserializing members: {}", e);
                    rusqlite::Error::InvalidQuery {}
                })?;
                Ok(members)
            })
            .optional()?;

        if members.is_some() {
            trace!("Found block {} members", block_hash);
        } else {
            trace!("Members not found");
        };

        Ok(members)
    }

    fn map_block() -> impl FnOnce(&Row) -> Result<Block, rusqlite::Error> {
        |row| {
            let body: Vec<u8> = row.get(0)?;
            let block = serde_json::from_slice::<Block>(&body).map_err(|e| {
                error!("Error deserializing block: {}", e);
                rusqlite::Error::InvalidQuery {}
            })?;
            Ok(block)
        }
    }
}
