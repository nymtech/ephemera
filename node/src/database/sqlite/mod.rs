use crate::block::Block;
use crate::config::configuration::DbConfig;
use crate::database::sqlite::query::DbQuery;
use crate::database::sqlite::store::DbStore;
use crate::database::EphemeraDatabase;

pub(crate) mod query;
pub(crate) mod store;

pub(crate) struct SqliteStorage {
    pub(crate) db_store: DbStore,
    pub(crate) db_query: DbQuery,
}

impl SqliteStorage {
    pub fn new(config: DbConfig) -> Self {
        let db_store = DbStore::open(config.clone()).unwrap();
        let db_query = DbQuery::open(config);
        Self { db_store, db_query }
    }

    pub(crate) fn get_block_by_id(&self, block_id: String) -> anyhow::Result<Option<Block>> {
        self.db_query.get_block_by_id(block_id)
    }

    pub(crate) fn store_block(&mut self, block: &Block) -> anyhow::Result<()> {
        self.db_store.store_block(block)
    }

    pub(crate) fn get_last_block(&self) -> anyhow::Result<Option<Block>> {
        self.db_query.get_last_block()
    }
}

impl EphemeraDatabase for SqliteStorage {
    fn get_block_by_id(&self, block_id: String) -> anyhow::Result<Option<Block>> {
        self.get_block_by_id(block_id)
    }

    fn store_block(&mut self, block: &Block) -> anyhow::Result<()> {
        self.store_block(block)
    }

    fn get_last_block(&self) -> anyhow::Result<Option<Block>> {
        self.get_last_block()
    }
}
