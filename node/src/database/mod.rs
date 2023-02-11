use crate::block::Block;
use crate::config::configuration::DbConfig;
use crate::utilities::crypto::Signature;

pub(crate) mod rocksdb;
pub(crate) mod sqlite;

//TODO: segregate API access and core ephemera database access
pub(crate) trait EphemeraDatabase {
    fn get_block_by_id(&self, block_id: String) -> anyhow::Result<Option<Block>>;

    fn store_block(&mut self, block: &Block, signatures: Vec<Signature>) -> anyhow::Result<()>;

    fn get_last_block(&self) -> anyhow::Result<Option<Block>>;

    fn get_block_by_label(&self, label: &str) -> anyhow::Result<Option<Block>>;

    fn get_block_signatures(&self, block_id: String) -> anyhow::Result<Option<Vec<Signature>>>;
}

pub(super) struct CompoundDatabase {
    pub(crate) sqlite: sqlite::SqliteStorage,
    pub(crate) rocksdb: rocksdb::RocksDbStorage,
}

impl CompoundDatabase {
    pub fn new(db_conf: DbConfig) -> Self {
        let sqlite = sqlite::SqliteStorage::new(db_conf.clone());
        let rocksdb = rocksdb::RocksDbStorage::new(db_conf);
        Self { sqlite, rocksdb }
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

    fn store_block(&mut self, block: &Block, signatures: Vec<Signature>) -> anyhow::Result<()> {
        self.sqlite
            .db_store
            .store_block(block, signatures.clone())?;
        self.rocksdb.db_store.store_block(block, signatures)?;
        Ok(())
    }

    fn get_last_block(&self) -> anyhow::Result<Option<Block>> {
        let sqlite_block = EphemeraDatabase::get_last_block(&self.sqlite)?;
        log::trace!("sqlite last block: {:?}", sqlite_block);

        let rocksdb_block = EphemeraDatabase::get_last_block(&self.rocksdb)?;
        log::trace!("rocksdb last block: {:?}", rocksdb_block);

        assert_eq!(sqlite_block, rocksdb_block);
        Ok(sqlite_block)
    }

    fn get_block_by_label(&self, label: &str) -> anyhow::Result<Option<Block>> {
        let sqlite_block = EphemeraDatabase::get_block_by_label(&self.sqlite, label)?;
        log::trace!("sqlite_block: {:?}", sqlite_block);

        let rocksdb_block = EphemeraDatabase::get_block_by_label(&self.rocksdb, label)?;
        log::trace!("rocksdb_block: {:?}", rocksdb_block);

        assert_eq!(sqlite_block, rocksdb_block);
        Ok(sqlite_block)
    }

    fn get_block_signatures(&self, block_id: String) -> anyhow::Result<Option<Vec<Signature>>> {
        let sqlite_signatures =
            EphemeraDatabase::get_block_signatures(&self.sqlite, block_id.clone())?;
        log::trace!("sqlite_signatures: {:?}", sqlite_signatures);

        let rocksdb_signatures =
            EphemeraDatabase::get_block_signatures(&self.rocksdb, block_id.clone())?;
        log::trace!("rocksdb_signatures: {:?}", rocksdb_signatures);

        assert_eq!(sqlite_signatures, rocksdb_signatures);
        Ok(sqlite_signatures)
    }
}
