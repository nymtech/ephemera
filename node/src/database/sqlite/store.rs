use crate::block::types::block::Block;
use anyhow::Result;
use rusqlite::{params, Connection};

use crate::config::DbConfig;
use crate::utilities::crypto::Signature;

mod migrations {
    use refinery::embed_migrations;

    embed_migrations!("migrations");
}

pub struct DbStore {
    connection: Connection,
}

impl DbStore {
    pub fn open(conf: DbConfig) -> Result<DbStore> {
        log::info!("Starting db backend with path: {}", conf.sqlite_path);

        let mut connection = Connection::open(conf.sqlite_path)?;
        DbStore::run_migrations(&mut connection)?;

        Ok(DbStore { connection })
    }

    pub(crate) fn store_block(&mut self, block: &Block, signatures: Vec<Signature>) -> Result<()> {
        log::debug!("Storing block: {}", block.header);

        let id = block.header.id.clone();
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

    pub fn run_migrations(connection: &mut Connection) -> Result<()> {
        log::info!("Running database migrations");
        match migrations::migrations::runner().run(connection) {
            Ok(ok) => {
                log::info!("Database migrations completed:{:?} ", ok);
                Ok(())
            }
            Err(err) => {
                log::error!("Database migrations failed: {}", err);
                Err(anyhow::anyhow!(err))
            }
        }
    }
}
