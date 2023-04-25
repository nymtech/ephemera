use std::sync::Arc;

use log::info;
use rocksdb::{TransactionDB, TransactionDBOptions};

use crate::block::types::block::Block;
use crate::config::DatabaseConfiguration;
use crate::storage::rocksdb::query::DbQuery;
use crate::storage::rocksdb::store::DbStore;
use crate::storage::EphemeraDatabase;
use crate::utilities::crypto::Certificate;

pub(crate) mod query;
pub(crate) mod store;

pub(crate) struct RocksDbStorage {
    pub(crate) db_store: DbStore,
    pub(crate) db_query: DbQuery,
}

const PREFIX_LAST_BLOCK_KEY: &str = "last_block";
const PREFIX_BLOCK_HASH: &str = "block_hash";
const PREFIX_BLOCK_HEIGHT: &str = "block_height";
const PREFIX_CERTIFICATES: &str = "block_certificates";

impl RocksDbStorage {
    pub fn open(db_conf: DatabaseConfiguration) -> anyhow::Result<Self> {
        info!("Opening RocksDB database at {}", db_conf.rocksdb_path);

        let mut options = rocksdb::Options::default();
        options.create_if_missing(db_conf.create_if_not_exists);

        let db = TransactionDB::open(
            &options,
            &TransactionDBOptions::default(),
            db_conf.rocksdb_path.clone(),
        )?;
        let db = Arc::new(db);
        let db_store = DbStore::new(db.clone());
        let db_query = DbQuery::new(db);
        let storage = Self { db_store, db_query };

        info!("Opened RocksDB database at {}", db_conf.rocksdb_path);
        Ok(storage)
    }
}

impl EphemeraDatabase for RocksDbStorage {
    fn get_block_by_id(&self, block_id: &str) -> anyhow::Result<Option<Block>> {
        self.db_query.get_block_by_hash(block_id)
    }

    fn get_last_block(&self) -> anyhow::Result<Option<Block>> {
        self.db_query.get_last_block()
    }

    fn get_block_by_height(&self, height: u64) -> anyhow::Result<Option<Block>> {
        self.db_query.get_block_by_height(height)
    }

    fn get_block_certificates(&self, block_id: &str) -> anyhow::Result<Option<Vec<Certificate>>> {
        self.db_query.get_block_certificates(block_id)
    }

    fn store_block(&mut self, block: &Block, certificates: &[Certificate]) -> anyhow::Result<()> {
        self.db_store.store_block(block, certificates)
    }
}

fn block_hash_key(block_hash: &str) -> String {
    format!("{PREFIX_BLOCK_HASH}:{block_hash}")
}

fn block_height_key(height: &u64) -> String {
    format!("{PREFIX_BLOCK_HEIGHT}:{height}")
}

fn last_block_key() -> String {
    PREFIX_LAST_BLOCK_KEY.to_string()
}

fn certificates_key(block_hash: &str) -> String {
    format!("{PREFIX_CERTIFICATES}:{block_hash}",)
}
