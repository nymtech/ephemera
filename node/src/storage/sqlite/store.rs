use crate::block::types::block::Block;
use anyhow::Result;
use rusqlite::{params, Connection, OpenFlags};

use crate::config::DbConfig;
use crate::utilities::crypto::Certificate;

pub struct DbStore {
    connection: Connection,
}

impl DbStore {
    pub fn open(db_conf: DbConfig, flags: OpenFlags) -> Result<DbStore> {
        let connection = Connection::open_with_flags(db_conf.sqlite_path, flags)?;
        Ok(DbStore { connection })
    }

    pub(crate) fn store_block(
        &mut self,
        block: &Block,
        signatures: Vec<Certificate>,
    ) -> Result<()> {
        log::debug!("Storing block: {}", block.header);

        let id = block.header.hash.to_string();
        let height = block.header.height;
        let block_bytes = serde_json::to_vec::<Block>(block).map_err(|e| anyhow::anyhow!(e))?;
        let signatures_bytes = serde_json::to_vec(&signatures).map_err(|e| anyhow::anyhow!(e))?;

        let tx = self.connection.transaction()?;
        {
            let mut statement = tx.prepare_cached(
                "INSERT INTO blocks (block_id, height, block) VALUES (?1, ?2, ?3)",
            )?;
            statement.execute(params![&id, &height, &block_bytes,])?;

            let mut statement =
                tx.prepare_cached("INSERT INTO signatures (block_id, signatures) VALUES (?1, ?2)")?;
            statement.execute(params![&id, &signatures_bytes,])?;
        }

        tx.commit()?;

        Ok(())
    }
}
