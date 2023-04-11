//! # Ephemera API
//!
//! This module contains all the types and functions available as part of the Ephemera public API.
//!
//! This API is also available over HTTP.

use std::fmt::Display;

use thiserror::Error;
use tokio::sync::{
    mpsc::{channel, Receiver, Sender},
    oneshot,
};

use crate::api::application::ApplicationError;
use crate::api::types::{ApiBlock, ApiCertificate, ApiEphemeraConfig, ApiEphemeraMessage};

pub(crate) mod application;
pub(crate) mod http;
pub(crate) mod types;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Application rejected ephemera message")]
    ApplicationRejectedMessage,
    #[error("Duplicate message")]
    DuplicateMessage,
    #[error("Invalid block hash: {0}")]
    InvalidBlockHash(#[from] bs58::decode::Error),
    #[error("ApplicationError: {0}")]
    Application(#[from] ApplicationError),
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

pub(crate) type DhtKV = (Vec<u8>, Vec<u8>);

#[derive(Debug)]
pub(crate) enum ApiCmd {
    SubmitEphemeraMessage(
        Box<ApiEphemeraMessage>,
        oneshot::Sender<Result<(), ApiError>>,
    ),
    QueryBlockByHeight(u64, oneshot::Sender<Result<Option<ApiBlock>, ApiError>>),
    QueryBlockById(String, oneshot::Sender<Result<Option<ApiBlock>, ApiError>>),
    QueryLastBlock(oneshot::Sender<Result<ApiBlock, ApiError>>),
    QueryBlockCertificates(
        String,
        oneshot::Sender<Result<Option<Vec<ApiCertificate>>, ApiError>>,
    ),
    QueryDht(Vec<u8>, oneshot::Sender<Result<Option<DhtKV>, ApiError>>),
    StoreInDht(Vec<u8>, Vec<u8>, oneshot::Sender<Result<(), ApiError>>),
    EphemeraConfig(oneshot::Sender<Result<ApiEphemeraConfig, ApiError>>),
}

impl Display for ApiCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiCmd::SubmitEphemeraMessage(message, _) => {
                write!(f, "SubmitEphemeraMessage({message})",)
            }
            ApiCmd::QueryBlockByHeight(height, _) => write!(f, "QueryBlockByHeight({height})",),
            ApiCmd::QueryBlockById(id, _) => write!(f, "QueryBlockById({id})",),
            ApiCmd::QueryLastBlock(_) => write!(f, "QueryLastBlock"),
            ApiCmd::QueryBlockCertificates(id, _) => write!(f, "QueryBlockSignatures{id}"),
            ApiCmd::QueryDht(_, _) => {
                write!(f, "QueryDht")
            }
            ApiCmd::StoreInDht(_, _, _) => {
                write!(f, "StoreInDht")
            }
            ApiCmd::EphemeraConfig(_) => {
                write!(f, "EphemeraConfig")
            }
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
    pub async fn get_block_certificates(
        &self,
        block_hash: String,
    ) -> Result<Option<Vec<ApiCertificate>>, ApiError> {
        log::trace!("get_block_certificates({})", block_hash);
        self.send_and_wait_response(|tx| ApiCmd::QueryBlockCertificates(block_hash, tx))
            .await
    }

    pub async fn query_dht(&self, key: Vec<u8>) -> Result<Option<(Vec<u8>, Vec<u8>)>, ApiError> {
        log::trace!("get_dht()");
        //TODO: this needs timeout(somewhere around dht query functionality)
        self.send_and_wait_response(|tx| ApiCmd::QueryDht(key, tx))
            .await
    }

    pub async fn store_in_dht(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), ApiError> {
        log::trace!("store_in_dht()");
        self.send_and_wait_response(|tx| ApiCmd::StoreInDht(key, value, tx))
            .await
    }

    pub async fn get_node_config(&self) -> Result<ApiEphemeraConfig, ApiError> {
        log::trace!("get_node_config()");
        self.send_and_wait_response(ApiCmd::EphemeraConfig).await
    }

    /// Send a message to Ephemera which should then be included in mempool  and broadcast to all peers
    pub async fn send_ephemera_message(&self, message: ApiEphemeraMessage) -> Result<(), ApiError> {
        log::trace!("send_ephemera_message({})", message);
        self.send_and_wait_response(|tx| ApiCmd::SubmitEphemeraMessage(message.into(), tx))
            .await
    }

    async fn send_and_wait_response<F, R>(&self, f: F) -> Result<R, ApiError>
    where
        F: FnOnce(oneshot::Sender<Result<R, ApiError>>) -> ApiCmd,
        R: Send + 'static,
    {
        let (tx, rcv) = oneshot::channel();
        let cmd = f(tx);
        if let Err(err) = self.commands_channel.send(cmd).await {
            return Err(ApiError::Internal(err.into()));
        }
        rcv.await.map_err(|e| {
            ApiError::Internal(anyhow::Error::msg(format!(
                "Failed to receive response from Ephemera: {e:?}"
            )))
        })?
    }
}
