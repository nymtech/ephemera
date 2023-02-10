//! This a basic implementation of a broadcast_protocol where participating peers go through three rounds to
//! reach a consensus on if/when to deliver a message.
//!
//! PRE-PREPARE:
//!      1. Upon receiving pre-prepare message, peer sends PREPARE message to all other peers.
//!
//! PREPARE:
//!     1. When peer receives a PREPARE message, it sends PREPARE message to all other peers.
//! COMMIT:
//!     1. When peer receives a quorum of PREPARE messages, it sends COMMIT message to all peers.
//!     2. When peer receives quorum of COMMIT messages, it considers the message as committed and can deliver it.
//!
//! It is possible to register a callback which will be called for the following events:
//!     1. Pre-prepare message received
//!     2. Prepare message received
//!     3. Prepare quorum reached
//!     4. Commit message received
//!     5. Commit quorum reached
//!
//! Callback can return optional (modified) message payload which is sent along with consensus message
//!  instead of the original payload from client.
//!

//! Limitations:
//! - Prepare and commit messages can reach out of order due to network and node processing delays. Nevertheless,
//!   a peer won't commit a message until it receives a quorum of prepare messages.
//! - Current implementation makes only progress(updates its state machine) when it receives a message from another peer.
//!   If for some reason messages are lost, the broadcast_protocol will not make progress. This can be fixed by introducing a timer and some concept
//!   of views/epoch.
//! - It doesn't try to total order different messages. All messages reach quorum consensus independently.
//!   All it does is that a quorum or no quorum of peers deliver the message.
//! - It doesn't verify the other peers authenticity.
//!   Also this can be a task for an upstream layer(gossip...) which handles networking and peers relationship.
//!
use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::time::SystemTime;

use lru::LruCache;
use thiserror::Error;
use tokio::time::Instant;

use crate::block::Block;
use crate::broadcast::broadcast_callback::BroadcastCallBack;
use crate::broadcast::broadcaster::BroadcastError::InvalidBroadcast;
use crate::broadcast::quorum::{BasicQuorum, Quorum};
use crate::broadcast::ReliableBroadcast::{Ack, Commit, Init, Prepare};
use crate::broadcast::{BroadcastData, Command, PeerId, RbMsg, ReliableBroadcast, Status};
use crate::config::configuration::Configuration;
use crate::utilities;
use crate::utilities::EphemeraId;

#[derive(Debug, Clone)]
pub struct ConsensusTimestamp(Instant);

impl Default for ConsensusTimestamp {
    fn default() -> Self {
        ConsensusTimestamp(Instant::now())
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct ConsensusContext {
    /// Peers that sent prepare message(this peer included)
    pub(crate) prepare: HashSet<PeerId>,
    /// Peers that sent commit message(this peer included)
    pub(crate) commit: HashSet<PeerId>,
    /// Flag indicating if the message has received enough prepare messages
    pub(crate) prepared: bool,
    /// Flag indicating if the message has received enough commit messages
    pub(crate) committed: bool,
}

impl ConsensusContext {
    pub(crate) fn new() -> ConsensusContext {
        ConsensusContext::default()
    }

    fn add_prepare(&mut self, peer: PeerId) {
        self.prepare.insert(peer);
    }

    fn add_commit(&mut self, peer: PeerId) {
        self.commit.insert(peer);
    }
}

#[derive(Error, Debug)]
pub enum BroadcastError {
    #[error("Invalid broadcast message {}", .0)]
    InvalidBroadcast(String),
    #[error("Duplicate pre-prepare message {}", .0)]
    CallbackError(#[from] anyhow::Error),
}

pub(crate) struct Broadcaster<C: BroadcastCallBack + Send> {
    contexts: LruCache<String, ConsensusContext>,
    quorum: BasicQuorum,
    callback: C,
    peer_id: PeerId,
}

#[derive(Debug)]
pub(crate) struct ProtocolResponse {
    pub(crate) status: Status,
    pub(crate) command: Command,
    pub(crate) protocol_reply: RbMsg<Block>,
}

pub(crate) type ProtocolResult = Result<ProtocolResponse, BroadcastError>;

impl<C: BroadcastCallBack<Message = Block>> Broadcaster<C> {
    pub fn new(config: Configuration, callback: C, peer_id: PeerId) -> Broadcaster<C> {
        Broadcaster {
            contexts: LruCache::new(NonZeroUsize::new(1000).unwrap()),
            quorum: BasicQuorum::new(config.quorum),
            callback,
            peer_id,
        }
    }

    pub async fn new_broadcast(&mut self, data: Block) -> ProtocolResult {
        log::debug!("Starting protocol for new block {}", data);
        let rb_msg = RbMsg::new(data, self.peer_id);

        self.contexts
            .get_or_insert(rb_msg.id.clone(), ConsensusContext::new);

        self.handle(rb_msg).await
    }

    pub async fn handle_broadcast_from_net(&mut self, rb_msg: RbMsg<Block>) -> ProtocolResult {
        if !self.contexts.contains(&rb_msg.id) && rb_msg.data.is_none() {
            return Err(InvalidBroadcast(
                "Received unknown broadcast without data".to_string(),
            ));
        }

        self.contexts
            .get_or_insert(rb_msg.id.clone(), ConsensusContext::new);
        match &rb_msg.data {
            Some(block) => {
                log::debug!(
                    "Received message {}-{}, block {} from {}",
                    rb_msg.id,
                    rb_msg.request_id,
                    block,
                    rb_msg.original_sender
                );
            }
            None => {
                log::debug!(
                    "Received message {}-{} from {}",
                    rb_msg.id,
                    rb_msg.request_id,
                    rb_msg.original_sender
                );
            }
        }
        let reply = self.handle(rb_msg).await;

        match &reply {
            Ok(ProtocolResponse {
                status: _,
                command: _,
                protocol_reply,
            }) => match &protocol_reply.reliable_broadcast {
                Init => {
                    log::debug!(
                        "Replying with INIT {}-{}",
                        protocol_reply.id,
                        protocol_reply.request_id
                    );
                }
                Prepare => {
                    log::debug!(
                        "Replying with PREPARE {}-{}",
                        protocol_reply.id,
                        protocol_reply.request_id
                    );
                }
                Commit => {
                    log::debug!(
                        "Replying with COMMIT {}-{}",
                        protocol_reply.id,
                        protocol_reply.request_id
                    );
                }
                Ack => {
                    log::debug!(
                        "Replying with ACK {}-{}",
                        protocol_reply.id,
                        protocol_reply.request_id
                    );
                }
            },
            Err(e) => {
                log::debug!("Replying with error {}", e);
            }
        }
        reply
    }

    async fn handle(&mut self, rb_msg: RbMsg<Block>) -> ProtocolResult {
        match rb_msg.reliable_broadcast.clone() {
            Init => {
                log::trace!("Processing INIT {}", rb_msg.id);
                self.process_init(rb_msg).await
            }
            Prepare => {
                log::trace!("Processing PREPARE {}", rb_msg.id);
                self.process_prepare(rb_msg).await
            }
            Commit => {
                log::trace!("Processing COMMIT {}", rb_msg.id);
                self.process_commit(rb_msg).await
            }
            Ack => self.ack(rb_msg.id, rb_msg.data_identifier),
        }
    }

    async fn process_init(&mut self, rb_msg: RbMsg<Block>) -> ProtocolResult {
        let msg_id = rb_msg.id.clone();

        self.contexts
            .get_mut(&rb_msg.id)
            .unwrap()
            .add_prepare(self.peer_id);

        let ctx = self.contexts.get(&rb_msg.id).unwrap();

        let data = rb_msg.data.expect("Data missing, this is a bug");
        let callback_result = self.callback.pre_prepare(&data, ctx).await?;

        let message = callback_result.unwrap_or(data);

        self.prepare_reply(msg_id, message.get_id(), Some(message), Command::Broadcast)
    }

    async fn process_prepare(&mut self, rb_msg: RbMsg<Block>) -> ProtocolResult {
        let mut ctx = self.contexts.get_mut(&rb_msg.id).unwrap();
        if ctx.prepared {
            log::trace!("Already prepared for {}", rb_msg.id);
            return self.ack(rb_msg.id.clone(), rb_msg.data_identifier);
        }

        ctx.add_prepare(rb_msg.original_sender);

        let data = rb_msg.data.expect("Data missing, this is a bug");
        let callback_result = self
            .callback
            .prepare(rb_msg.original_sender, &data, ctx)
            .await?;
        let data = callback_result.unwrap_or(data);

        //This block is executed only by followers(who didn't receive pre-prepare)
        if !ctx.prepare.contains(&self.peer_id) {
            ctx.add_prepare(self.peer_id);

            return self.prepare_reply(
                rb_msg.id.clone(),
                rb_msg.data_identifier,
                Some(data),
                Command::Broadcast,
            );
        }

        if self.quorum.prepare_threshold(ctx.prepare.len()) {
            log::trace!("Prepare completed for {}", rb_msg.id);

            ctx.prepared = true;
            self.callback.prepared(ctx).await?;

            ctx.add_commit(self.peer_id);

            return self.commit_reply(
                rb_msg.id.clone(),
                false,
                Command::Broadcast,
                rb_msg.data_identifier,
                Some(data),
            );
        }

        self.ack(rb_msg.id.clone(), data.get_id())
    }

    async fn process_commit(&mut self, rb_msg: RbMsg<Block>) -> ProtocolResult {
        let mut ctx = self.contexts.get_mut(&rb_msg.id).unwrap();
        if ctx.committed {
            log::trace!("Commit already completed for {}", rb_msg.id);
            return self.ack(rb_msg.id, rb_msg.data_identifier);
        }

        ctx.commit.insert(rb_msg.original_sender);

        if !ctx.commit.contains(&self.peer_id) {
            ctx.commit.insert(self.peer_id);
        }

        self.callback.commit(&rb_msg.original_sender, ctx).await?;

        if ctx.prepared && self.quorum.commit_threshold(ctx.commit.len()) {
            log::trace!("Commit complete for {}", rb_msg.id);

            ctx.committed = true;
            self.callback.committed(ctx).await?;
        }

        let committed = ctx.committed;
        self.commit_reply(
            rb_msg.id,
            committed,
            Command::Broadcast,
            rb_msg.data_identifier,
            rb_msg.data,
        )
    }

    pub(crate) fn broadcast_reply(
        &self,
        id: String,
        msg: ReliableBroadcast,
        status: Status,
        command: Command,
        data_identifier: EphemeraId,
        data: Option<Block>,
    ) -> ProtocolResult {
        Ok(ProtocolResponse {
            status,
            command,
            protocol_reply: RbMsg {
                id,
                request_id: utilities::generate_ephemera_id(),
                original_sender: self.peer_id,
                data_identifier,
                timestamp: SystemTime::now(),
                data,
                reliable_broadcast: msg,
            },
        })
    }

    pub(crate) fn prepare_reply(
        &self,
        id: String,
        data_identifier: EphemeraId,
        data: Option<Block>,
        command: Command,
    ) -> ProtocolResult {
        self.broadcast_reply(id, Prepare, Status::Pending, command, data_identifier, data)
    }

    pub(crate) fn commit_reply(
        &self,
        id: String,
        committed: bool,
        command: Command,
        data_identifier: EphemeraId,
        data: Option<Block>,
    ) -> ProtocolResult {
        let state = if committed {
            Status::Committed
        } else {
            Status::Pending
        };
        self.broadcast_reply(id, Commit, state, command, data_identifier, data)
    }

    pub(crate) fn ack(&self, id: String, data_identifier: EphemeraId) -> ProtocolResult {
        self.broadcast_reply(
            id,
            Ack,
            Status::Finished,
            Command::Drop,
            data_identifier,
            None,
        )
    }
}
