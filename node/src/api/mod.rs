//! # Ephemera API
//!
//! This module contains all the types and functions available as part of the Ephemera public API.
//!
//! This API is also available over HTTP.

pub(crate) mod application;
pub(crate) mod http;
pub(crate) mod types;

use log::trace;
use std::fmt::Display;
use tokio::sync::{
    mpsc::{channel, Receiver, Sender},
    oneshot,
};

use crate::api::types::{
    ApiBlock, ApiBroadcastInfo, ApiCertificate, ApiEphemeraConfig, ApiEphemeraMessage, ApiError,
};

/// Kademlia DHT key
pub(crate) type DhtKey = Vec<u8>;

/// Kademlia DHT value
pub(crate) type DhtValue = Vec<u8>;

/// Kademlia DHT key/value pair
pub(crate) type DhtKV = (DhtKey, DhtValue);

pub(crate) type Result<T> = std::result::Result<T, ApiError>;

#[derive(Debug)]
pub(crate) enum ToEphemeraApiCmd {
    SubmitEphemeraMessage(Box<ApiEphemeraMessage>, oneshot::Sender<Result<()>>),
    QueryBlockByHeight(u64, oneshot::Sender<Result<Option<ApiBlock>>>),
    QueryBlockById(String, oneshot::Sender<Result<Option<ApiBlock>>>),
    QueryLastBlock(oneshot::Sender<Result<ApiBlock>>),
    QueryBlockCertificates(String, oneshot::Sender<Result<Option<Vec<ApiCertificate>>>>),
    QueryDht(DhtKey, oneshot::Sender<Result<Option<DhtKV>>>),
    StoreInDht(DhtKey, DhtValue, oneshot::Sender<Result<()>>),
    EphemeraConfig(oneshot::Sender<Result<ApiEphemeraConfig>>),
    BroadcastGroup(oneshot::Sender<Result<ApiBroadcastInfo>>),
}

impl Display for ToEphemeraApiCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToEphemeraApiCmd::SubmitEphemeraMessage(message, _) => {
                write!(f, "SubmitEphemeraMessage({message})",)
            }
            ToEphemeraApiCmd::QueryBlockByHeight(height, _) => write!(f, "QueryBlockByHeight({height})",),
            ToEphemeraApiCmd::QueryBlockById(id, _) => write!(f, "QueryBlockById({id})",),
            ToEphemeraApiCmd::QueryLastBlock(_) => write!(f, "QueryLastBlock"),
            ToEphemeraApiCmd::QueryBlockCertificates(id, _) => write!(f, "QueryBlockSignatures{id}"),
            ToEphemeraApiCmd::QueryDht(_, _) => {
                write!(f, "QueryDht")
            }
            ToEphemeraApiCmd::StoreInDht(_, _, _) => {
                write!(f, "StoreInDht")
            }
            ToEphemeraApiCmd::EphemeraConfig(_) => {
                write!(f, "EphemeraConfig")
            }
            ToEphemeraApiCmd::BroadcastGroup(_) => {
                write!(f, "BroadcastGroup")
            }
        }
    }
}

pub(crate) struct ApiListener {
    pub(crate) messages_rcv: Receiver<ToEphemeraApiCmd>,
}

impl ApiListener {
    pub(crate) fn new(messages_rcv: Receiver<ToEphemeraApiCmd>) -> Self {
        Self { messages_rcv }
    }
}

#[derive(Clone)]
pub struct CommandExecutor {
    pub(crate) commands_channel: Sender<ToEphemeraApiCmd>,
}

impl CommandExecutor {
    pub(crate) fn new() -> (CommandExecutor, ApiListener) {
        let (commands_channel, signed_messages_rcv) = channel(100);
        let api_listener = ApiListener::new(signed_messages_rcv);
        let api = CommandExecutor { commands_channel };
        (api, api_listener)
    }

    /// Returns block with given id if it exists
    ///
    /// # Arguments
    ///
    /// * `block_id` - Block id
    ///
    /// # Errors
    ///
    /// * `ApiError::InternalError` - If there is an internal error
    pub async fn get_block_by_id(&self, block_id: String) -> Result<Option<ApiBlock>> {
        trace!("get_block_by_id({:?})", block_id);
        self.send_and_wait_response(|tx| ToEphemeraApiCmd::QueryBlockById(block_id, tx))
            .await
    }

    /// Returns block with given height if it exists
    ///
    /// # Arguments
    ///
    /// * `height` - Block height
    ///
    /// # Errors
    ///
    /// * `ApiError::InternalError` - If there is an internal error
    pub async fn get_block_by_height(&self, height: u64) -> Result<Option<ApiBlock>> {
        trace!("get_block_by_height({:?})", height);
        self.send_and_wait_response(|tx| ToEphemeraApiCmd::QueryBlockByHeight(height, tx))
            .await
    }

    /// Returns last block. Which has maximum height and is stored in database
    ///
    /// # Errors
    ///
    /// * `ApiError::InternalError` - If there is an internal error
    pub async fn get_last_block(&self) -> Result<ApiBlock> {
        trace!("get_last_block()");
        self.send_and_wait_response(ToEphemeraApiCmd::QueryLastBlock).await
    }

    /// Returns signatures for given block id
    ///
    /// # Arguments
    ///
    /// * `block_hash` - Block id
    ///
    /// # Errors
    ///
    /// * `ApiError::InternalError` - If there is an internal error
    pub async fn get_block_certificates(
        &self,
        block_hash: String,
    ) -> Result<Option<Vec<ApiCertificate>>> {
        trace!("get_block_certificates({block_hash:?})",);
        self.send_and_wait_response(|tx| ToEphemeraApiCmd::QueryBlockCertificates(block_hash, tx))
            .await
    }

    /// Queries DHT for given key
    ///
    /// # Arguments
    ///
    /// * `key` - DHT key
    ///
    /// # Errors
    ///
    /// * `ApiError::InternalError` - If there is an internal error
    ///
    /// # Returns
    ///
    /// * `Some((key, value))` - If key is found
    /// * `None` - If key is not found
    pub async fn query_dht(&self, key: DhtKey) -> Result<Option<(DhtKey, DhtValue)>> {
        trace!("get_dht({key:?})");
        //TODO: this needs timeout(somewhere around dht query functionality)
        self.send_and_wait_response(|tx| ToEphemeraApiCmd::QueryDht(key, tx))
            .await
    }

    /// Stores given key-value pair in DHT
    ///
    /// # Arguments
    ///
    /// * `key` - DHT key
    /// * `value` - DHT value
    ///
    /// # Errors
    ///
    /// * `ApiError::InternalError` - If there is an internal error
    pub async fn store_in_dht(&self, key: DhtKey, value: DhtValue) -> Result<()> {
        trace!("store_in_dht({key:?}, {value:?})");
        self.send_and_wait_response(|tx| ToEphemeraApiCmd::StoreInDht(key, value, tx))
            .await
    }

    /// Returns node configuration
    ///
    /// # Errors
    ///
    /// * `ApiError::InternalError` - If there is an internal error
    ///
    /// # Returns
    ///
    /// * `ApiEphemeraConfig` - Node configuration
    pub async fn get_node_config(&self) -> Result<ApiEphemeraConfig> {
        trace!("get_node_config()");
        self.send_and_wait_response(ToEphemeraApiCmd::EphemeraConfig).await
    }

    /// Returns broadcast group
    ///
    /// # Errors
    ///
    /// * `ApiError::InternalError` - If there is an internal error
    ///
    /// # Returns
    ///
    /// * `ApiBroadcastInfo` - Broadcast group
    pub async fn get_broadcast_info(&self) -> Result<ApiBroadcastInfo> {
        trace!("get_broadcast_group()");
        self.send_and_wait_response(ToEphemeraApiCmd::BroadcastGroup).await
    }

    /// Send a message to Ephemera which should then be included in mempool  and broadcast to all peers
    ///
    /// # Arguments
    ///
    /// * `message` - Message to be sent
    ///
    /// # Errors
    ///
    /// * `ApiError::InternalError` - If there is an internal error
    pub async fn send_ephemera_message(&self, message: ApiEphemeraMessage) -> Result<()> {
        trace!("send_ephemera_message({message})",);
        self.send_and_wait_response(|tx| ToEphemeraApiCmd::SubmitEphemeraMessage(message.into(), tx))
            .await
    }

    async fn send_and_wait_response<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(oneshot::Sender<Result<R>>) -> ToEphemeraApiCmd,
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
