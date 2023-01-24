use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::time;

///! This a basic implementation of a broadcast_protocol where participating peers go through three rounds to
///! reach a consensus on if/when to deliver a message.
///!
///! PRE-PREPARE:
///      1. Upon receiving pre-prepare message, peer sends PREPARE message to all other peers.
///!
///! PREPARE:
///!     1. When peer receives a PREPARE message, it sends PREPARE message to all other peers.
///! COMMIT:
///!     1. When peer receives a quorum of PREPARE messages, it sends COMMIT message to all peers.
///      2. When peer receives quorum of COMMIT messages, it considers the message as committed and can deliver it.
///!
///! It is possible to register a callback which will be called for the following events:
///!     1. Pre-prepare message received
///!     2. Prepare message received
///!     3. Prepare quorum reached
///!     4. Commit message received
///!     5. Commit quorum reached
///!
///! Callback can return optional (modified) message payload which is sent along with consensus message
///!  instead of the original payload from client.
///!

///! Limitations:
///! - Prepare and commit messages can reach out of order due to network and node processing delays. Nevertheless,
///!   a peer won't commit a message until it receives a quorum of prepare messages.
///! - Current implementation makes only progress(updates its state machine) when it receives a message from another peer.
///!   If for some reason messages are lost, the broadcast_protocol will not make progress. This can be fixed by introducing a timer and some concept
///!   of views/epoch.
///! - It doesn't try to total order different messages. All messages reach quorum consensus independently.
///!   All it does is that a quorum or no quorum of peers deliver the message.
///! - It doesn't verify the other peers authenticity.
///!   Also this can be a task for an upstream layer(gossip...) which handles networking and peers relationship.
///!
use lru::LruCache;
use prost_types::Timestamp;
use thiserror::Error;
use tokio::time::Instant;

use crate::broadcast_protocol::broadcast::BroadcastError::UnknownBroadcast;
use crate::broadcast_protocol::quorum::Quorum;
use crate::broadcast_protocol::{BroadcastCallBack, EphemeraSigningRequest, Kind, ProtocolResponse};
use crate::config::configuration::Configuration;
use crate::request::rb_msg::ReliableBroadcast;
use crate::request::rb_msg::ReliableBroadcast::{Ack, Commit, PrePrepare, Prepare};
use crate::request::{AckMsg, CommitMsg, PrePrepareMsg, PrepareMsg, RbMsg};

#[derive(Debug, Clone)]
pub struct ConsensusTimestamp(Instant);

impl Default for ConsensusTimestamp {
    fn default() -> Self {
        ConsensusTimestamp(Instant::now())
    }
}

#[derive(Debug, Default, Clone)]
pub struct ConsensusContext {
    pub id: String,
    pub timestamp: ConsensusTimestamp,
    pub original_sender: bool,
    pub local_address: String,
    pub prepare: HashSet<String>,
    pub commit: HashSet<String>,
    pub prepared: bool,
    pub committed: bool,
}

impl ConsensusContext {
    pub fn new(id: String, original_sender: bool, local_address: String) -> ConsensusContext {
        ConsensusContext {
            id,
            timestamp: ConsensusTimestamp::default(),
            original_sender,
            local_address,
            ..ConsensusContext::default()
        }
    }

    fn add_prepare(&mut self, peer: String) {
        self.prepare.insert(peer);
    }

    fn add_commit(&mut self, peer: String) {
        self.commit.insert(peer);
    }
}

#[derive(Error, Debug)]
pub enum BroadcastError {
    #[error("Unknown broadcast_protocol {}", .0)]
    UnknownBroadcast(String),
    #[error("Duplicate pre-prepare message {}", .0)]
    DuplicateMessage(String),
    #[error(transparent)]
    CallbackError(#[from] anyhow::Error),
}

pub struct BroadcastProtocol<C: BroadcastCallBack + Send> {
    pub(crate) contexts: LruCache<String, ConsensusContext>,
    quorum: Box<dyn Quorum + Send>,
    callback: C,
    node_id: String,
}

pub(crate) type ProtocolResult = Result<ProtocolResponse, BroadcastError>;

impl<C: BroadcastCallBack + Send> BroadcastProtocol<C> {
    pub fn new(quorum: Box<dyn Quorum + Send>, callback: C, config: Configuration) -> BroadcastProtocol<C> {
        BroadcastProtocol {
            contexts: LruCache::new(NonZeroUsize::new(1000).unwrap()),
            quorum,
            callback,
            node_id: config.node_config.address,
        }
    }

    pub async fn handle(&mut self, pr_msg: EphemeraSigningRequest) -> ProtocolResult {
        let rb_msg = pr_msg
            .message
            .reliable_broadcast
            .ok_or_else(|| UnknownBroadcast("Unknown broadcast_protocol".to_string()))?;

        let rb_id = pr_msg.message.id;
        let node_id = pr_msg.message.node_id;
        let custom_message_id = pr_msg.message.custom_message_id;

        match rb_msg {
            PrePrepare(PrePrepareMsg { payload }) => {
                self.process_pre_prepare(rb_id, custom_message_id, node_id, payload)
                    .await
            }
            Prepare(PrepareMsg { payload }) => {
                self.process_prepare(rb_id, custom_message_id, node_id, payload)
                    .await
            }
            Commit(_) => self.process_commit(rb_id, custom_message_id, node_id).await,
            Ack(_) => ack(rb_id, custom_message_id, node_id),
        }
    }

    async fn process_pre_prepare(
        &mut self,
        msg_id: String,
        custom_message_id: String,
        sender: String,
        payload: Vec<u8>,
    ) -> ProtocolResult {
        log::debug!("Received pre-prepare message {} from {}", msg_id, sender);

        if self.contexts.contains(&msg_id) {
            return Err(BroadcastError::DuplicateMessage(msg_id));
        }

        let mut ctx = ConsensusContext::new(msg_id.clone(), true, self.node_id.clone());

        ctx.add_prepare(self.node_id.clone());

        let callback_result = self
            .callback
            .pre_prepare(
                msg_id.clone(),
                custom_message_id.clone(),
                sender,
                payload.clone(),
                &ctx,
            )
            .await?;

        let payload = callback_result.unwrap_or(payload);

        self.contexts.put(msg_id.clone(), ctx);

        prepare_reply(msg_id, custom_message_id, self.node_id.clone(), payload)
    }

    async fn process_prepare(
        &mut self,
        msg_id: String,
        custom_message_id: String,
        sender: String,
        payload: Vec<u8>,
    ) -> ProtocolResult {
        log::debug!("Received prepare message {} from {}", msg_id, sender);

        let mut ctx = self.contexts.get_or_insert_mut(msg_id.clone(), || {
            ConsensusContext::new(msg_id.clone(), false, self.node_id.clone())
        });

        if ctx.prepared {
            return ack(msg_id.clone(), custom_message_id, self.node_id.clone());
        }

        ctx.add_prepare(sender.to_owned());

        let callback_result = self
            .callback
            .prepare(
                msg_id.clone(),
                custom_message_id.clone(),
                sender,
                payload.clone(),
                ctx,
            )
            .await?;
        let payload = callback_result.unwrap_or(payload);

        if !ctx.prepare.contains(&self.node_id) {
            log::debug!("Sending prepare for {}", msg_id);

            ctx.add_prepare(self.node_id.clone());
        }

        if self.quorum.prepare_threshold(ctx.prepare.len()) {
            log::debug!("Prepare completed for {}", msg_id);

            ctx.prepared = true;
            self.callback.prepared(ctx).await?;

            if ctx.original_sender {
                log::debug!("Sending commit for {}", msg_id);

                ctx.add_commit(self.node_id.clone());

                return commit_reply(msg_id.clone(), custom_message_id, self.node_id.clone());
            }
        }

        prepare_reply(msg_id.clone(), custom_message_id, self.node_id.clone(), payload)
    }

    async fn process_commit(
        &mut self,
        msg_id: String,
        custom_message_id: String,
        origin: String,
    ) -> ProtocolResult {
        log::debug!("Received commit message {} from {}", msg_id, origin);

        let mut ctx = self
            .contexts
            .get_mut(&msg_id)
            .ok_or(UnknownBroadcast(msg_id.clone()))?;

        if ctx.committed {
            log::debug!("Commit already completed for {}", msg_id);
            return ack(msg_id.clone(), custom_message_id, self.node_id.clone());
        }

        ctx.commit.insert(origin.to_owned());

        if !ctx.commit.contains(&self.node_id) {
            ctx.commit.insert(self.node_id.clone());
        }

        log::debug!("COMMIT COUNT {}", ctx.commit.len());
        self.callback
            .commit(msg_id.clone(), custom_message_id.clone(), origin, ctx)
            .await?;

        log::debug!("CTX {:?}", ctx);
        if ctx.prepared && self.quorum.commit_threshold(ctx.commit.len()) {
            log::debug!("Commit complete for {}", msg_id);

            ctx.committed = true;
            self.callback.committed(ctx).await?;
        }
        commit_reply(msg_id.clone(), custom_message_id, self.node_id.clone())
    }
}

pub(crate) fn broadcast_reply(
    id: String,
    custom_message_id: String,
    node_id: String,
    msg: ReliableBroadcast,
    kind: Kind,
) -> ProtocolResult {
    let timestamp = Some(Timestamp::from(time::SystemTime::now()));
    Ok(ProtocolResponse {
        kind,
        protocol_reply: RbMsg {
            id,
            node_id,
            timestamp,
            custom_message_id,
            reliable_broadcast: Some(msg),
        },
    })
}

pub(crate) fn prepare_reply(
    id: String,
    custom_message_id: String,
    node_id: String,
    payload: Vec<u8>,
) -> ProtocolResult {
    let msg = Prepare(PrepareMsg { payload });
    broadcast_reply(id, custom_message_id, node_id, msg, Kind::Broadcast)
}

pub(crate) fn commit_reply(id: String, custom_message_id: String, node_id: String) -> ProtocolResult {
    let msg = Commit(CommitMsg {});
    broadcast_reply(id, custom_message_id, node_id, msg, Kind::Broadcast)
}

pub(crate) fn ack(id: String, custom_message_id: String, node_id: String) -> ProtocolResult {
    let msg = Ack(AckMsg {});
    broadcast_reply(id, custom_message_id, node_id, msg, Kind::Drop)
}
