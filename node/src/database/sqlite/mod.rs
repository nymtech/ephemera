use crate::block::Block;
use crate::config::configuration::DbConfig;
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
    pub fn new(config: DbConfig) -> Self {
        let db_store = DbStore::open(config.clone()).unwrap();
        let db_query = DbQuery::open(config);
        Self { db_store, db_query }
    }

    pub(crate) fn get_block_by_id(&self, block_id: String) -> anyhow::Result<Option<Block>> {
        self.db_query.get_block_by_id(block_id)
    }

    pub(crate) fn store_block(
        &mut self,
        block: &Block,
        signatures: Vec<Signature>,
    ) -> anyhow::Result<()> {
        self.db_store.store_block(block, signatures)
    }

    pub(crate) fn get_last_block(&self) -> anyhow::Result<Option<Block>> {
        self.db_query.get_last_block()
    }

    pub(crate) fn get_block_by_label(&self, label: &str) -> anyhow::Result<Option<Block>> {
        self.db_query.get_block_by_label(label)
    }
}

impl EphemeraDatabase for SqliteStorage {
    fn get_block_by_id(&self, block_id: String) -> anyhow::Result<Option<Block>> {
        self.get_block_by_id(block_id)
    }

    fn store_block(&mut self, block: &Block, signatures: Vec<Signature>) -> anyhow::Result<()> {
        self.store_block(block, signatures)
    }

    fn get_last_block(&self) -> anyhow::Result<Option<Block>> {
        self.get_last_block()
    }

    fn get_block_by_label(&self, label: &str) -> anyhow::Result<Option<Block>> {
        self.get_block_by_label(label)
    }
}
