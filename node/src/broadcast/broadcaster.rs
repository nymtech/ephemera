use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::sync::Arc;

use crate::block::types::block::Block;
use lru::LruCache;
use thiserror::Error;

use crate::broadcast::broadcaster::BroadcastError::InvalidBroadcast;
use crate::broadcast::quorum::{BasicQuorum, Quorum};
use crate::broadcast::signing::BlockSigner;
use crate::broadcast::Phase::{Commit, Prepare};
use crate::broadcast::{Command, PeerId, RbMsg, Status};
use crate::config::Configuration;
use crate::utilities::crypto::ed25519::Ed25519Keypair;
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
    #[error("{}", .0)]
    General(#[from] anyhow::Error),
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
    pub fn new(
        config: Configuration,
        peer_id: PeerId,
        keypair: Arc<Ed25519Keypair>,
    ) -> Broadcaster {
        let block_signer = BlockSigner::new(keypair);
        Broadcaster {
            contexts: LruCache::new(NonZeroUsize::new(1000).unwrap()),
            quorum: BasicQuorum::new(config.quorum),
            peer_id,
            block_signer,
        }
    }

    pub(crate) async fn new_broadcast(
        &mut self,
        block: Block,
    ) -> Result<ProtocolResponse, BroadcastError> {
        log::debug!("Starting protocol for new block {}", block);
        let signature = self.block_signer.sign_block(block.clone())?;
        let rb_msg = RbMsg::new(block, self.peer_id, signature);
        self.handle(rb_msg).await
    }

    pub(crate) async fn handle(
        &mut self,
        rb_msg: RbMsg,
    ) -> Result<ProtocolResponse, BroadcastError> {
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
            _ => Err(InvalidBroadcast(format!(
                "Invalid broadcast message {}",
                rb_msg.id
            ))),
        };
        log::trace!("Context after new broadcast: {:?}", self.contexts.get(&id));
        result
    }

    async fn process_prepare(&mut self, rb_msg: RbMsg) -> Result<ProtocolResponse, BroadcastError> {
        let block = rb_msg.get_data().expect("Block should be present");
        let mut ctx = self
            .contexts
            .get_or_insert_mut(rb_msg.id.clone(), ConsensusContext::new);

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
        let mut ctx = self
            .contexts
            .get_or_insert_mut(rb_msg.id.clone(), ConsensusContext::new);

        if self.peer_id != rb_msg.original_sender {
            ctx.add_commit(rb_msg.original_sender);
        }

        let signature = self.block_signer.sign_block(block.clone())?;

        //Because we don't control in which order nodes receive prepare and commit messages
        //(because current implementation is really basic),
        //we add commit even if we haven't prepared yet. It still first waits enough prepare messages before it sends its commit message.
        if !ctx.commit.contains(&self.peer_id) {
            ctx.add_commit(self.peer_id);

            return Ok(ProtocolResponse {
                status: Status::Pending,
                command: Command::Broadcast,
                protocol_reply: rb_msg.commit_reply(self.peer_id, block.clone(), signature),
            });
        }

        if ctx.prepared && !ctx.committed && self.quorum.commit_threshold(ctx.commit.len()) {
            log::debug!("Commit complete for {}", rb_msg.id);
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
