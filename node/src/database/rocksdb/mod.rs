use std::sync::Arc;

use rocksdb::{TransactionDB, TransactionDBOptions};

use crate::block::types::block::Block;
use crate::config::DbConfig;
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

const PREFIX_LAST_BLOCK_KEY: &str = "last_block";
const PREFIX_BLOCK_ID: &str = "block_id";
const PREFIX_BLOCK_HEIGHT: &str = "block_height";
const PREFIX_SIGNATURES: &str = "signatures";

impl RocksDbStorage {
    pub fn open(db_conf: DbConfig) -> anyhow::Result<Self> {
        log::info!("Opening RocksDB database at {}", db_conf.rocket_path);

        let mut options = rocksdb::Options::default();
        if !db_conf.create_if_not_exists {
            options.create_if_missing(false)
        }

        let db = TransactionDB::open(
            &options,
            &TransactionDBOptions::default(),
            db_conf.rocket_path.clone(),
        )?;
        let db = Arc::new(db);
        let db_store = DbStore::new(db.clone());
        let db_query = DbQuery::new(db);
        let storage = Self { db_store, db_query };

        log::info!("Opened RocksDB database at {}", db_conf.rocket_path);
        Ok(storage)
    }
}

impl EphemeraDatabase for RocksDbStorage {
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

fn block_id_key(block_id: &str) -> String {
    format!("{PREFIX_BLOCK_ID}:{block_id}")
}

fn block_height_key(height: &u64) -> String {
    format!("{PREFIX_BLOCK_HEIGHT}:{height}")
}

fn last_block_key() -> String {
    PREFIX_LAST_BLOCK_KEY.to_string()
}

fn signatures_key(block_id: &str) -> String {
    format!("{PREFIX_SIGNATURES}:{PREFIX_BLOCK_ID}:{block_id}",)
}
