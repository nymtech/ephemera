use std::sync::Arc;

use rocksdb::TransactionDB;

use crate::block::Block;
use crate::config::configuration::DbConfig;
use crate::database::rocksdb::query::DbQuery;
use crate::database::rocksdb::store::DbStore;
use crate::database::EphemeraDatabase;
use crate::utilities::crypto::Signature;

pub(crate) mod query;
pub(crate) mod store;

pub(crate) struct RocksDbStorage {
    pub(crate) db_store: DbStore,
    pub(crate) db_query: DbQuery,
}

const LAST_BLOCK_KEY: &str = "last_block";
const PREFIX_BLOCK_ID: &str = "block_id";
const PREFIX_LABEL: &str = "label";
const PREFIX_SIGNATURES: &str = "signatures";

impl RocksDbStorage {
    pub fn new(config: DbConfig) -> Self {
        log::info!("Opening RocksDB database at {}", config.rocket_path);
        let db = TransactionDB::open_default(config.rocket_path.clone()).unwrap();
        log::info!("Opened RocksDB database at {}", config.rocket_path);

        let db = Arc::new(db);
        let db_store = DbStore::new(db.clone()).unwrap();
        let db_query = DbQuery::new(db).unwrap();
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

impl EphemeraDatabase for RocksDbStorage {
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
