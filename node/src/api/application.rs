use thiserror::Error;

use crate::api::types::{ApiBlock, ApiEphemeraMessage};

#[derive(Debug, Clone, PartialEq)]
pub enum RemoveMessages {
    /// Remove all messages from the mempool
    All,
    /// Remove only inclued messages from the mempool
    Selected(Vec<ApiEphemeraMessage>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CheckBlockResult {
    /// Accept the block
    Accept,
    /// Reject the block with a reason.
    Reject,
    /// Reject the block and remove messages from the mempool
    RejectAndRemoveMessages(RemoveMessages),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckBlockResponse {
    pub accept: bool,
    pub reason: Option<String>,
}

#[derive(Error, Debug)]
pub enum ApplicationError {
    //Just a placeholder for now
    #[error("ApplicationError::GeneralError: {0}")]
    GeneralError(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, ApplicationError>;

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
    fn check_tx(&self, message: ApiEphemeraMessage) -> Result<bool>;

    /// Ephemera produces new blocks with configured interval.
    /// Application can decide whether to accept the block or not.
    /// For example, if the block doesn't contain any transactions, it can be rejected.
    //TODO: maybe add more metadata, including list of peers which will be part of broadcast
    fn check_block(&self, block: &ApiBlock) -> Result<CheckBlockResult>;

    /// Deliver Block is called after block is confirmed by Ephemera and persisted to the storage.
    fn deliver_block(&self, block: ApiBlock) -> Result<()>;
}

#[derive(Default)]
pub struct DefaultApplication;

/// Default application which doesn't do any validation.
impl Application for DefaultApplication {
    fn check_tx(&self, tx: ApiEphemeraMessage) -> Result<bool> {
        log::trace!("ApplicationPlaceholder::check_tx: {tx:?}");
        Ok(true)
    }

    fn check_block(&self, block: &ApiBlock) -> Result<CheckBlockResult> {
        log::trace!("ApplicationPlaceholder::accept_block: {block:?}");
        Ok(CheckBlockResult::Accept)
    }

    fn deliver_block(&self, block: ApiBlock) -> Result<()> {
        log::trace!("ApplicationPlaceholder::deliver_block: {block:?}");
        Ok(())
    }
}
