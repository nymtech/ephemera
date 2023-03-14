//! # Ephemera API
//!
//! This module contains all the types and functions available as part of the Ephemera public API.
//! This API is also available over HTTP.
use std::fmt::Display;

use thiserror::Error;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::oneshot;

use crate::api::types::{ApiBlock, ApiEphemeraMessage, ApiSignature};

pub mod application;
pub mod types;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("{0}")]
    ApiError(String),
}

#[derive(Debug)]
pub(crate) enum ApiCmd {
    SubmitEphemeraMessage(ApiEphemeraMessage),
    QueryBlockByHeight(u64, oneshot::Sender<Result<Option<ApiBlock>, ApiError>>),
    QueryBlockById(String, oneshot::Sender<Result<Option<ApiBlock>, ApiError>>),
    QueryLastBlock(oneshot::Sender<Result<ApiBlock, ApiError>>),
    QueryBlockSignatures(
        String,
        oneshot::Sender<Result<Option<Vec<ApiSignature>>, ApiError>>,
    ),
}

impl Display for ApiCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiCmd::SubmitEphemeraMessage(message) => {
                write!(f, "SubmitEphemeraMessage({message})",)
            }
            ApiCmd::QueryBlockByHeight(height, _) => write!(f, "QueryBlockByHeight({height})",),
            ApiCmd::QueryBlockById(id, _) => write!(f, "QueryBlockById({id})",),
            ApiCmd::QueryLastBlock(_) => write!(f, "QueryLastBlock"),
            ApiCmd::QueryBlockSignatures(id, _) => write!(f, "QueryBlockSignatures{id}"),
        }
    }
}

pub(crate) struct ApiListener {
    pub(crate) messages_rcv: Receiver<ApiCmd>,
}

impl ApiListener {
    pub(crate) fn new(messages_rcv: Receiver<ApiCmd>) -> Self {
        Self { messages_rcv }
    }
}

#[derive(Clone)]
pub struct EphemeraExternalApi {
    pub(crate) commands_channel: Sender<ApiCmd>,
}

impl EphemeraExternalApi {
    pub(crate) fn new() -> (EphemeraExternalApi, ApiListener) {
        let (commands_channel, signed_messages_rcv) = channel(100);
        let api_listener = ApiListener::new(signed_messages_rcv);
        let api = EphemeraExternalApi { commands_channel };
        (api, api_listener)
    }

    /// Returns block with given id if it exists
    pub async fn get_block_by_id(&self, block_id: String) -> Result<Option<ApiBlock>, ApiError> {
        log::trace!("get_block_by_id({})", block_id);
        self.send_and_wait_response(|tx| ApiCmd::QueryBlockById(block_id, tx))
            .await
    }

    /// Returns block with given height if it exists
    pub async fn get_block_by_height(&self, height: u64) -> Result<Option<ApiBlock>, ApiError> {
        log::trace!("get_block_by_height({})", height);
        self.send_and_wait_response(|tx| ApiCmd::QueryBlockByHeight(height, tx))
            .await
    }

    /// Returns last block. Which has maximum height and is stored in database
    pub async fn get_last_block(&self) -> Result<ApiBlock, ApiError> {
        log::trace!("get_last_block()");
        self.send_and_wait_response(ApiCmd::QueryLastBlock).await
    }

    /// Returns signatures for given block id
    pub async fn get_block_signatures(
        &self,
        block_id: String,
    ) -> Result<Option<Vec<ApiSignature>>, ApiError> {
        log::trace!("get_block_by_height({block_id})",);
        self.send_and_wait_response(|tx| ApiCmd::QueryBlockSignatures(block_id, tx))
            .await
    }

    /// Send a message to Ephemera which should then be included in mempool  and broadcast to all peers
    pub async fn send_ephemera_message(&self, message: ApiEphemeraMessage) -> Result<(), ApiError> {
        log::trace!("send_ephemera_message({})", message);
        let cmd = ApiCmd::SubmitEphemeraMessage(message);
        self.commands_channel.send(cmd).await.map_err(|_| {
            ApiError::ApiError(
                "Api channel closed. It means that probably Ephemera is crashed".to_string(),
            )
        })
    }

    async fn send_and_wait_response<F, R>(&self, f: F) -> Result<R, ApiError>
    where
        F: FnOnce(oneshot::Sender<Result<R, ApiError>>) -> ApiCmd,
        R: Send + 'static,
    {
        let (tx, rcv) = oneshot::channel();
        let cmd = f(tx);
        if let Err(err) = self.commands_channel.send(cmd).await {
            log::error!("Failed to send command to Ephemera: {:?}", err);
            return Err(ApiError::ApiError(
                "Api channel closed. It means that probably Ephemera is crashed".to_string(),
            ));
        }
        rcv.await.map_err(|e| {
            log::error!("Failed to receive response from Ephemera: {:?}", e);
            ApiError::ApiError("Failed to receive response from Ephemera".to_string())
        })?
    }
}
