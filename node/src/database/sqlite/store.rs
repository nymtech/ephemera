use anyhow::Result;
use rusqlite::{params, Connection};

use crate::block::Block;
use crate::config::configuration::DbConfig;

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

    pub(crate) fn store_block(&self, block: &Block) -> Result<()> {
        log::debug!("Storing block: {}", block.header);

        let id = block.header.id.clone();
        let body = serde_json::to_vec(&block).map_err(|e| anyhow::anyhow!(e))?;
        let mut statement = self
            .connection
            .prepare_cached("INSERT INTO blocks (block_id, block) VALUES (?1, ?2)")?;
        statement.execute(params![&id, &body,])?;
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
