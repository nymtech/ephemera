//! # Database
//!
//! It supports `SqlLite` and `RocksDB`.
//!
//! ## `RocksDB`
//!
//! `SqlLite` is used by default.
//!
//! ## `SqlLite`
//!
//! To use `SqlLite`, you need to compile with the `sqlite_storage` feature and with `--no-default-features` flag.

#[cfg(feature = "rocksdb_storage")]
pub(crate) mod rocksdb;

#[cfg(feature = "sqlite_storage")]
pub(crate) mod sqlite;

use crate::block::types::block::Block;
use crate::peer::PeerId;
use crate::utilities::crypto::Certificate;
use std::collections::HashSet;

pub(crate) trait EphemeraDatabase: Send {
    /// Returns block by its id. Block ids are generated by Ephemera
    fn get_block_by_id(&self, block_id: &str) -> anyhow::Result<Option<Block>>;

    /// Returns last committed/finalised block.
    fn get_last_block(&self) -> anyhow::Result<Option<Block>>;

    /// Returns block by its height
    fn get_block_by_height(&self, height: u64) -> anyhow::Result<Option<Block>>;

    /// Returns block certificates.
    ///
    /// Certificates were created as part of broadcast protocol and signed by peers who participated.
    fn get_block_certificates(&self, block_id: &str) -> anyhow::Result<Option<Vec<Certificate>>>;

    /// Returns peers who participated in block broadcast.
    fn get_block_broadcast_group(&self, block_id: &str) -> anyhow::Result<Option<Vec<PeerId>>>;

    /// Stores block and its signatures
    fn store_block(
        &mut self,
        block: &Block,
        certificates: HashSet<Certificate>,
        members: HashSet<PeerId>,
    ) -> anyhow::Result<()>;
}
