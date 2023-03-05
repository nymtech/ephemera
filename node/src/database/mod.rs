//! # Database
//!
//! It supports SqlLite and RocksDB. RocksDB seems a more reasonable choice in
//! long term as Ephemera data access pattern is very well defined and it doesn't need
//! the flexibility of SQL(which comes with cost). Also relying here on the experience
//! of other similar projects.
//!
//! Of cource we can also make either of them as optional feature

use crate::block::types::block::Block;
use crate::config::DbConfig;
use crate::utilities::crypto::Signature;

pub(crate) mod rocksdb;
pub(crate) mod sqlite;

pub(crate) trait EphemeraDatabase {
    /// Returns block by its id. Block ids are generated by Ephemera
    fn get_block_by_id(&self, block_id: String) -> anyhow::Result<Option<Block>>;
    /// Returns last committed/finalised block.
    fn get_last_block(&self) -> anyhow::Result<Option<Block>>;
    /// Returns block by its height
    fn get_block_by_height(&self, height: u64) -> anyhow::Result<Option<Block>>;
    fn get_block_signatures(&self, block_id: String) -> anyhow::Result<Option<Vec<Signature>>>;
    /// Stores block and its signatures
    fn store_block(&mut self, block: &Block, signatures: Vec<Signature>) -> anyhow::Result<()>;
}

pub(super) struct CompoundDatabase {
    pub(crate) sqlite: sqlite::SqliteStorage,
    pub(crate) rocksdb: rocksdb::RocksDbStorage,
}

impl CompoundDatabase {
    pub fn open(db_conf: DbConfig) -> anyhow::Result<Self> {
        let rocksdb = rocksdb::RocksDbStorage::open(db_conf.clone())?;
        let sqlite = sqlite::SqliteStorage::open(db_conf)?;
        let database = Self { sqlite, rocksdb };
        Ok(database)
    }
}

impl EphemeraDatabase for CompoundDatabase {
    fn get_block_by_id(&self, block_id: String) -> anyhow::Result<Option<Block>> {
        let sqlite_block = EphemeraDatabase::get_block_by_id(&self.sqlite, block_id.clone())?;
        log::trace!("sqlite_block: {:?}", sqlite_block);

        let rocksdb_block = EphemeraDatabase::get_block_by_id(&self.rocksdb, block_id)?;
        log::trace!("rocksdb_block: {:?}", rocksdb_block);

        assert_eq!(sqlite_block, rocksdb_block);
        Ok(sqlite_block)
    }

    fn get_last_block(&self) -> anyhow::Result<Option<Block>> {
        let sqlite_block = EphemeraDatabase::get_last_block(&self.sqlite)?;
        log::trace!("sqlite last block: {:?}", sqlite_block);

        let rocksdb_block = EphemeraDatabase::get_last_block(&self.rocksdb)?;
        log::trace!("rocksdb last block: {:?}", rocksdb_block);

        assert_eq!(sqlite_block, rocksdb_block);
        Ok(sqlite_block)
    }

    fn get_block_by_height(&self, height: u64) -> anyhow::Result<Option<Block>> {
        let sqlite_block = EphemeraDatabase::get_block_by_height(&self.sqlite, height)?;
        log::trace!("sqlite_block: {:?}", sqlite_block);

        let rocksdb_block = EphemeraDatabase::get_block_by_height(&self.rocksdb, height)?;
        log::trace!("rocksdb_block: {:?}", rocksdb_block);

        assert_eq!(sqlite_block, rocksdb_block);
        Ok(sqlite_block)
    }

    fn get_block_signatures(&self, block_id: String) -> anyhow::Result<Option<Vec<Signature>>> {
        let sqlite_signatures =
            EphemeraDatabase::get_block_signatures(&self.sqlite, block_id.clone())?;
        log::trace!("sqlite_signatures: {:?}", sqlite_signatures);

        let rocksdb_signatures = EphemeraDatabase::get_block_signatures(&self.rocksdb, block_id)?;
        log::trace!("rocksdb_signatures: {:?}", rocksdb_signatures);

        assert_eq!(sqlite_signatures, rocksdb_signatures);
        Ok(sqlite_signatures)
    }

    fn store_block(&mut self, block: &Block, signatures: Vec<Signature>) -> anyhow::Result<()> {
        EphemeraDatabase::store_block(&mut self.rocksdb, block, signatures.clone())?;
        EphemeraDatabase::store_block(&mut self.sqlite, block, signatures)?;
        Ok(())
    }
}
