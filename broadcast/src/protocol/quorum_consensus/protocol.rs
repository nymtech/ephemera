use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::time;

///! This a basic implementation of a protocol where participating peers go through three rounds to
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
///!  PS! At the moment it chooses the list of peers who will receive the messages. Actually this should be the responsibility
///!      of the broadcasting/gossiping part of the system. Current design sees this protocol and gossip
///!      protocol semantically equivalent but they seem to be actually different part of the stack like IP and TCP.
///!
///! Limitations:
///! - Prepare and commit messages can reach out of order due to network and node processing delays. Nevertheless,
///!   a peer won't commit a message until it receives a quorum of prepare messages.
///! - Current implementation makes only progress(updates its state machine) when it receives a message from another peer.
///!   If for some reason messages are lost, the protocol will not make progress. This can be fixed by introducing a timer and some concept
///!   of views/epoch.
///! - It doesn't try to total order different messages. All messages reach quorum consensus independently.
///!   All it does is that a quorum or no quorum of peers deliver the message.
///! - It doesn't verify the other peers authenticity.
///!   Also this can be a task for an upstream layer(gossip...) which handles networking and peers relationship.
///!
///!
use lru::LruCache;
use prost_types::Timestamp;
use thiserror::Error;
use tokio::time::Instant;


use crate::protocol::{Kind, Protocol, ProtocolRequest, ProtocolResponse};
use crate::protocol::quorum_consensus::protocol::QuorumProtocolError::UnknownBroadcast;
use crate::protocol::quorum_consensus::quorum::Quorum;
use crate::protocol::quorum_consensus::quorum_consensus_callback::QuorumConsensusCallBack;
use crate::request::rb_msg::ReliableBroadcast;
use crate::request::rb_msg::ReliableBroadcast::{Ack, Commit, PrePrepare, Prepare};
use crate::request::{AckMsg, CommitMsg, PrePrepareMsg, PrepareMsg, RbMsg};
use crate::settings::Settings;

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
pub enum QuorumProtocolError {
    #[error("Unknown broadcast {}", .0)]
    UnknownBroadcast(String),
    #[error("Duplicate pre-prepare message {}", .0)]
    DuplicateMessage(String),
    #[error(transparent)]
    CallbackError(#[from] anyhow::Error),
}

pub struct QuorumConsensusBroadcastProtocol<Req, Res> {
    pub(crate) contexts: LruCache<String, ConsensusContext>,
    quorum: Box<dyn Quorum + Send>,
    callback: Box<dyn QuorumConsensusCallBack<Req, Res> + Send>,
    node_id: String,
}

type ProtocolResult = Result<ProtocolResponse<RbMsg>, QuorumProtocolError>;
type Callback = Box<dyn QuorumConsensusCallBack<RbMsg, RbMsg> + Send>;

#[async_trait::async_trait]
impl Protocol<RbMsg> for QuorumConsensusBroadcastProtocol<RbMsg, RbMsg> {
    type Response = RbMsg;
    type Error = QuorumProtocolError;

    async fn handle(&mut self, msg: ProtocolRequest<RbMsg>) -> ProtocolResult {
        self.handle_message(msg).await
    }
}

impl QuorumConsensusBroadcastProtocol<RbMsg, RbMsg> {
    pub fn new(
        quorum: Box<dyn Quorum + Send>,
        callback: Callback,
        settings: Settings,
    ) -> QuorumConsensusBroadcastProtocol<RbMsg, RbMsg> {
        QuorumConsensusBroadcastProtocol {
            contexts: LruCache::new(NonZeroUsize::new(1000).unwrap()),
            quorum,
            callback,
            node_id: settings.address,
        }
    }

    pub(crate) async fn handle_message(&mut self, pr_msg: ProtocolRequest<RbMsg>) -> ProtocolResult {
        let rb_msg = pr_msg
            .message
            .reliable_broadcast
            .ok_or_else(|| UnknownBroadcast("Unknown broadcast".to_string()))?;

        let rb_id = pr_msg.message.id;
        let node_id = pr_msg.message.node_id;

        match rb_msg {
            PrePrepare(PrePrepareMsg { payload }) => self.process_pre_prepare(rb_id, node_id, payload),
            Prepare(PrepareMsg { payload }) => self.process_prepare(rb_id, node_id, payload),
            Commit(_) => self.process_commit(rb_id, node_id),
            Ack(_) => ack(rb_id, node_id),
        }
    }

    fn process_pre_prepare(&mut self, msg_id: String, sender: String, payload: Vec<u8>) -> ProtocolResult {
        log::debug!("Received pre-prepare message {} from {}", msg_id, sender);

        if self.contexts.contains(&msg_id) {
            return Err(QuorumProtocolError::DuplicateMessage(msg_id));
        }

        let mut ctx = ConsensusContext::new(msg_id.clone(), true, self.node_id.clone());

        ctx.add_prepare(self.node_id.clone());

        let callback_result = self
            .callback
            .pre_prepare(msg_id.clone(), sender, payload.clone(), &ctx)?;

        let payload = callback_result.unwrap_or(payload);

        self.contexts.put(msg_id.clone(), ctx);

        prepare_reply(msg_id, self.node_id.clone(), payload)
    }

    fn process_prepare(&mut self, msg_id: String, sender: String, payload: Vec<u8>) -> ProtocolResult {
        log::debug!("Received prepare message {} from {}", msg_id, sender);

        let mut ctx = self.contexts.get_or_insert_mut(msg_id.clone(), || {
            ConsensusContext::new(msg_id.clone(), false, self.node_id.clone())
        });

        if ctx.prepared {
            return ack(msg_id.clone(), self.node_id.clone());
        }

        ctx.add_prepare(sender.to_owned());

        let callback_result = self
            .callback
            .prepare(msg_id.clone(), sender, payload.clone(), ctx)?;
        let payload = callback_result.unwrap_or(payload);

        if !ctx.prepare.contains(&self.node_id) {
            log::debug!("Sending prepare for {}", msg_id);

            ctx.add_prepare(self.node_id.clone());
            return prepare_reply(msg_id.clone(), self.node_id.clone(), payload);
        }

        if self.quorum.prepare_threshold(ctx.prepare.len() as u64) {
            log::debug!("Prepare completed for {}", msg_id);

            ctx.prepared = true;
            self.callback.prepared(ctx)?;

            if ctx.original_sender {
                log::debug!("Sending commit for {}", msg_id);

                ctx.add_commit(self.node_id.clone());

                return commit_reply(msg_id.clone(), self.node_id.clone());
            }
        }

        ack(msg_id.clone(), self.node_id.clone())
    }

    fn process_commit(&mut self, msg_id: String, origin: String) -> ProtocolResult {
        log::debug!("Received commit message {} from {}", msg_id, origin);

        let mut ctx = self
            .contexts
            .get_mut(&msg_id)
            .ok_or(QuorumProtocolError::UnknownBroadcast(msg_id.clone()))?;

        if ctx.committed {
            return ack(msg_id.clone(), self.node_id.clone());
        }

        ctx.commit.insert(origin.to_owned());

        if !ctx.commit.contains(&self.node_id) {
            ctx.commit.insert(self.node_id.clone());
            return commit_reply(msg_id.clone(), self.node_id.clone());
        }

        self.callback.commit(msg_id.clone(), origin, ctx)?;

        if ctx.prepared && self.quorum.commit_threshold(ctx.commit.len() as u64) {
            log::debug!("Commit complete for {}", msg_id);

            ctx.committed = true;
            self.callback.committed(ctx)?;
        }
        ack(msg_id, self.node_id.clone())
    }
}

pub(crate) fn broadcast_reply(
    id: String,
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
            reliable_broadcast: Some(msg),
        },
    })
}

pub(crate) fn prepare_reply(id: String, node_id: String, payload: Vec<u8>) -> ProtocolResult {
    let msg = Prepare(PrepareMsg { payload });
    broadcast_reply(id, node_id, msg, Kind::Broadcast)
}

pub(crate) fn commit_reply(id: String, node_id: String) -> ProtocolResult {
    let msg = Commit(CommitMsg {});
    broadcast_reply(id, node_id, msg, Kind::Broadcast)
}

pub(crate) fn ack(id: String, node_id: String) -> ProtocolResult {
    let msg = Ack(AckMsg {});
    broadcast_reply(id, node_id, msg, Kind::Drop)
}
