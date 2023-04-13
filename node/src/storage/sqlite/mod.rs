use rusqlite::Connection;

use crate::block::types::block::Block;
use crate::config::DbConfig;
use crate::storage::sqlite::query::DbQuery;
use crate::storage::sqlite::store::DbStore;
use crate::storage::EphemeraDatabase;
use crate::utilities::crypto::Certificate;

pub(crate) mod query;
pub(crate) mod store;

mod migrations {
    use refinery::embed_migrations;

    embed_migrations!("migrations");
}

pub(crate) struct SqliteStorage {
    pub(crate) db_store: DbStore,
    pub(crate) db_query: DbQuery,
}

impl SqliteStorage {
    pub fn open(db_conf: DbConfig) -> anyhow::Result<Self> {
        let mut flags = rusqlite::OpenFlags::default();
        if !db_conf.create_if_not_exists {
            flags.remove(rusqlite::OpenFlags::SQLITE_OPEN_CREATE);
        }

        let mut connection = Connection::open_with_flags(db_conf.sqlite_path.clone(), flags)?;
        Self::run_migrations(&mut connection)?;

        log::info!("Starting db backend with path: {}", db_conf.sqlite_path);
        let db_store = DbStore::open(db_conf.clone(), flags)?;
        let db_query = DbQuery::open(db_conf, flags)?;
        let storage = Self { db_store, db_query };
        Ok(storage)
    }

    pub fn run_migrations(connection: &mut Connection) -> anyhow::Result<()> {
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

impl EphemeraDatabase for SqliteStorage {
    fn get_block_by_id(&self, block_id: String) -> anyhow::Result<Option<Block>> {
        self.db_query.get_block_by_hash(block_id)
    }

    fn get_last_block(&self) -> anyhow::Result<Option<Block>> {
        self.db_query.get_last_block()
    }

    fn get_block_by_height(&self, height: u64) -> anyhow::Result<Option<Block>> {
        self.db_query.get_block_by_height(height)
    }

    fn get_block_certificates(&self, block_id: String) -> anyhow::Result<Option<Vec<Certificate>>> {
        self.db_query.get_block_certificates(block_id)
    }

    fn store_block(&mut self, block: &Block, certificates: Vec<Certificate>) -> anyhow::Result<()> {
        self.db_store.store_block(block, certificates)
    }
}
