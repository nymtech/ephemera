use crate::api::types::{ApiBlock, ApiEphemeraMessage};

///Cosmos style ABCI application hook
///
/// Notes:
/// A) These functions should be relatively fast, as they are called synchronously by Ephemera main loop.
/// B) They should not wait on a lock
/// C) They should not panic
pub trait Application {
    /// Check Tx is called upon receiving a new transaction from the mempool.
    /// It's up to the application to decide whether the transaction is valid or not.
    /// Basic check could for example be signature verification.
    fn check_tx(&self, message: ApiEphemeraMessage) -> anyhow::Result<bool>;

    /// Ephemera produces new blocks with configured interval.
    /// Application can decide whether to accept the block or not.
    /// For example, if the block doesn't contain any transactions, it can be rejected.
    //TODO: maybe add more metadata, including list of peers which will be part of broadcast
    fn check_block(&self, block: &ApiBlock) -> anyhow::Result<bool>;

    /// Deliver Block is called after block is confirmed by Ephemera and persisted to the storage.
    fn deliver_block(&self, block: ApiBlock) -> anyhow::Result<()>;
}

#[derive(Default)]
pub struct DefaultApplication;

/// Default application which doesn't do any validation.
impl Application for DefaultApplication {
    fn check_tx(&self, tx: ApiEphemeraMessage) -> anyhow::Result<bool> {
        log::info!("ApplicationPlaceholder::check_tx: {tx:?}");
        Ok(true)
    }

    fn check_block(&self, block: &ApiBlock) -> anyhow::Result<bool> {
        log::info!("ApplicationPlaceholder::accept_block: {block:?}");
        Ok(true)
    }

    fn deliver_block(&self, block: ApiBlock) -> anyhow::Result<()> {
        log::info!("ApplicationPlaceholder::deliver_block: {block:?}");
        Ok(())
    }
}
