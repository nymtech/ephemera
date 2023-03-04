use crate::block::types::block::Block;
use crate::config::DbConfig;
use crate::database::sqlite::query::DbQuery;
use crate::database::sqlite::store::DbStore;
use crate::database::EphemeraDatabase;
use crate::utilities::crypto::Signature;

pub(crate) mod query;
pub(crate) mod store;

pub(crate) struct SqliteStorage {
    pub(crate) db_store: DbStore,
    pub(crate) db_query: DbQuery,
}

impl SqliteStorage {
    pub fn open(config: DbConfig) -> anyhow::Result<Self> {
        let db_store = DbStore::open(config.clone())?;
        let db_query = DbQuery::open(config)?;
        let storage = Self { db_store, db_query };
        Ok(storage)
    }
}

impl EphemeraDatabase for SqliteStorage {
    fn get_block_by_id(&self, block_id: String) -> anyhow::Result<Option<Block>> {
        self.db_query.get_block_by_id(block_id)
    }

    fn get_last_block(&self) -> anyhow::Result<Option<Block>> {
        self.db_query.get_last_block()
    }

    fn get_block_by_height(&self, height: u64) -> anyhow::Result<Option<Block>> {
        self.db_query.get_block_by_height(height)
    }

    fn get_block_signatures(&self, block_id: String) -> anyhow::Result<Option<Vec<Signature>>> {
        self.db_query.get_block_signatures(block_id)
    }

    fn store_block(&mut self, block: &Block, signatures: Vec<Signature>) -> anyhow::Result<()> {
        self.db_store.store_block(block, signatures)
    }
}
