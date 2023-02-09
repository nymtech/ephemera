use crate::block::Block;
use crate::config::configuration::DbConfig;

pub(crate) mod rocksdb;
pub(crate) mod sqlite;

//TODO: segregate API access and core ephemera database access
pub(crate) trait EphemeraDatabase {
    fn get_block_by_id(&self, block_id: String) -> anyhow::Result<Option<Block>>;
    //TODO: doesn't belong here
    fn store_block(&mut self, block: &Block) -> anyhow::Result<()>;

    fn get_last_block(&self) -> anyhow::Result<Option<Block>>;

    fn get_block_by_label(&self, label: &str) -> anyhow::Result<Option<Block>>;
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
        let sqlite_block = self.sqlite.get_block_by_id(block_id.clone())?;
        log::trace!("sqlite_block: {:?}", sqlite_block);

        let rocksdb_block = self.rocksdb.get_block_by_id(block_id)?;
        log::trace!("rocksdb_block: {:?}", rocksdb_block);

        assert_eq!(sqlite_block, rocksdb_block);
        Ok(sqlite_block)
    }

    fn store_block(&mut self, block: &Block) -> anyhow::Result<()> {
        self.sqlite.store_block(block)?;
        self.rocksdb.store_block(block)?;
        Ok(())
    }

    fn get_last_block(&self) -> anyhow::Result<Option<Block>> {
        let sqlite_block = self.sqlite.get_last_block()?;
        log::trace!("sqlite last block: {:?}", sqlite_block);

        let rocksdb_block = self.rocksdb.get_last_block()?;
        log::trace!("rocksdb last block: {:?}", rocksdb_block);

        assert_eq!(sqlite_block, rocksdb_block);
        Ok(sqlite_block)
    }

    fn get_block_by_label(&self, label: &str) -> anyhow::Result<Option<Block>> {
        let sqlite_block = self.sqlite.get_block_by_label(label)?;
        log::trace!("sqlite_block: {:?}", sqlite_block);

        let rocksdb_block = self.rocksdb.get_block_by_label(label)?;
        log::trace!("rocksdb_block: {:?}", rocksdb_block);

        assert_eq!(sqlite_block, rocksdb_block);
        Ok(sqlite_block)
    }
}
