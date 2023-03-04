use crate::api::types::{ApiBlock, ApiEphemeraMessage};

///Cosmos style ABCI application hook, excluded unnecessary methods
///
/// Notes:
/// A) These functions should be relatively fast, as they are called synchronously.
/// B) They should not wait on a lock
/// C) They should not panic
///
/// Despite of the above, importance of it depends on the application which uses Ephemera as its library.
pub trait Application {
    /// CheckTx is called upon receiving a new transaction from the mempool.
    /// It's up to the application to decide whether the transaction is valid or not.
    /// Basic check could be signature verification.
    fn check_tx(&self, tx: ApiEphemeraMessage) -> anyhow::Result<bool>;

    /// Ephemera produces new blocks with configured interval regardless.
    /// Application can decide whether to accept the block or not.
    /// For example, if the block doesn't contain any transactions, it can be rejected.
    fn accept_block(&self, _block: &ApiBlock) -> anyhow::Result<bool>;

    /// DeliverBlock is called when block is confirmed by Ephemera and persisted to the storage.
    fn deliver_block(&self, _block: ApiBlock) -> anyhow::Result<()>;
}

#[derive(Default)]
pub struct DefaultApplication;

/// Default application which doesn't do any validation.
impl Application for DefaultApplication {
    fn check_tx(&self, tx: ApiEphemeraMessage) -> anyhow::Result<bool> {
        log::trace!("ApplicationPlaceholder::check_tx: {tx:?}");
        Ok(true)
    }

    fn accept_block(&self, block: &ApiBlock) -> anyhow::Result<bool> {
        log::trace!("ApplicationPlaceholder::accept_block: {block:?}");
        Ok(true)
    }

    fn deliver_block(&self, block: ApiBlock) -> anyhow::Result<()> {
        log::trace!("ApplicationPlaceholder::deliver_block: {block:?}");
        Ok(())
    }
}
