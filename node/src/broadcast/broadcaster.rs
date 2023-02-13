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
use std::sync::Arc;

use lru::LruCache;
use thiserror::Error;

use crate::block::Block;
use crate::broadcast::{Command, PeerId, RbMsg, Status};
use crate::broadcast::broadcaster::BroadcastError::InvalidBroadcast;
use crate::broadcast::Phase::{Commit, Prepare};
use crate::broadcast::quorum::{BasicQuorum, Quorum};
use crate::broadcast::signing::BlockSigner;
use crate::config::configuration::Configuration;
use crate::utilities::crypto::libp2p2_crypto::Libp2pKeypair;
use crate::utilities::crypto::Signature;

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
        ConsensusContext {
            prepare: HashSet::new(),
            commit: HashSet::new(),
            prepared: false,
            committed: false,
        }
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

pub(crate) struct Broadcaster {
    contexts: LruCache<String, ConsensusContext>,
    quorum: BasicQuorum,
    peer_id: PeerId,
    block_signer: BlockSigner,
}

#[derive(Debug)]
pub(crate) struct ProtocolResponse {
    pub(crate) status: Status,
    pub(crate) command: Command,
    pub(crate) protocol_reply: RbMsg,
}

impl Broadcaster {
    pub fn new(config: Configuration, peer_id: PeerId, keypair: Arc<Libp2pKeypair>) -> Broadcaster {
        let block_signer = BlockSigner::new(keypair);
        Broadcaster {
            contexts: LruCache::new(NonZeroUsize::new(1000).unwrap()),
            quorum: BasicQuorum::new(config.quorum),
            peer_id,
            block_signer,
        }
    }

    pub(crate) async fn new_broadcast(&mut self, block: Block) -> Result<ProtocolResponse, BroadcastError> {
        log::debug!("Starting protocol for new block {}", block);
        let signature = self.block_signer.sign_block(block.clone())?;
        let rb_msg = RbMsg::new(block, self.peer_id, signature);
        self.handle(rb_msg).await
    }

    pub(crate) async fn handle(&mut self, rb_msg: RbMsg) -> Result<ProtocolResponse, BroadcastError> {
        let block = rb_msg.get_data().expect("Block should be present");
        self.block_signer.verify_block(block, &rb_msg.signature)?;

        let id = rb_msg.id.clone();
        let result = match rb_msg.phase.clone() {
            Prepare(_) => {
                log::trace!("Processing PREPARE {}", rb_msg.id);
                self.process_prepare(rb_msg).await
            }
            Commit(_) => {
                log::trace!("Processing COMMIT {}", rb_msg.id);
                self.process_commit(rb_msg).await
            }
            _ => Err(InvalidBroadcast(format!("Invalid broadcast message {}", rb_msg.id))),
        };
        log::trace!("Context after new broadcast: {:?}", self.contexts.get(&id));
        result
    }

    async fn process_prepare(&mut self, rb_msg: RbMsg) -> Result<ProtocolResponse, BroadcastError> {
        let block = rb_msg.get_data().expect("Block should be present");
        let mut ctx = self.contexts.get_or_insert_mut(rb_msg.id.clone(), ConsensusContext::new);

        if self.peer_id != rb_msg.original_sender {
            ctx.add_prepare(rb_msg.original_sender);
        }

        let signature = self.block_signer.sign_block(block.clone())?;

        if !ctx.prepare.contains(&self.peer_id) {
            ctx.add_prepare(self.peer_id);

            return Ok(ProtocolResponse {
                status: Status::Pending,
                command: Command::Broadcast,
                protocol_reply: rb_msg.prepare_reply(self.peer_id, block.clone(), signature),
            });
        }

        if self.quorum.prepare_threshold(ctx.prepare.len()) {
            log::trace!("Prepare completed for {}", rb_msg.id);

            ctx.prepared = true;
            ctx.add_commit(self.peer_id);

            return Ok(ProtocolResponse {
                status: Status::Pending,
                command: Command::Broadcast,
                protocol_reply: rb_msg.commit_reply(self.peer_id, block.clone(), signature),
            });
        }

        Ok(ProtocolResponse {
            status: Status::Pending,
            command: Command::Drop,
            protocol_reply: rb_msg.ack_reply(self.peer_id, signature),
        })
    }

    async fn process_commit(&mut self, rb_msg: RbMsg) -> Result<ProtocolResponse, BroadcastError> {
        let block = rb_msg.get_data().expect("Block should be present");
        let mut ctx = self.contexts.get_or_insert_mut(rb_msg.id.clone(), ConsensusContext::new);

        if self.peer_id != rb_msg.original_sender {
            ctx.add_commit(rb_msg.original_sender);
        }

        let signature = self.block_signer.sign_block(block.clone())?;

        //Because we don't control in which order nodes receive prepare and commit messages, we do may do commit even if we haven't prepared yet
        if !ctx.commit.contains(&self.peer_id) {
            ctx.add_commit(self.peer_id);

            return Ok(ProtocolResponse {
                status: Status::Pending,
                command: Command::Broadcast,
                protocol_reply: rb_msg.commit_reply(self.peer_id, block.clone(), signature),
            });
        }

        if ctx.prepared && !ctx.committed && self.quorum.commit_threshold(ctx.commit.len()) {
            log::trace!("Commit complete for {}", rb_msg.id);
            ctx.committed = true;

            return Ok(ProtocolResponse {
                status: Status::Committed,
                command: Command::Broadcast,
                protocol_reply: rb_msg.commit_reply(self.peer_id, block.clone(), signature),
            });
        }

        Ok(ProtocolResponse {
            status: Status::Pending,
            command: Command::Drop,
            protocol_reply: rb_msg.ack_reply(self.peer_id, signature),
        })
    }

    pub(crate) fn get_block_signatures(&mut self, block_id: &str) -> Option<Vec<Signature>> {
        self.block_signer.get_block_signatures(block_id)
    }
}
