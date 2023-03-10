use std::num::NonZeroUsize;
use std::sync::Arc;

use lru::LruCache;

use crate::block::types::block::Block;
use crate::broadcast::bracha::quorum::BrachaQuorum;
use crate::broadcast::signing::BlockSigner;
use crate::broadcast::BroadcastError::InvalidBroadcast;
use crate::broadcast::MessageType::{Echo, Vote};
use crate::broadcast::{BroadcastError, Command, ConsensusContext, PeerId, Quorum, RbMsg, Status};
use crate::config::BroadcastConfig;
use crate::utilities::crypto::ed25519::Ed25519Keypair;
use crate::utilities::crypto::Signature;

pub(crate) struct Broadcaster {
    contexts: LruCache<String, ConsensusContext>,
    quorum: BrachaQuorum,
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
        config: BroadcastConfig,
        peer_id: PeerId,
        keypair: Arc<Ed25519Keypair>,
    ) -> Broadcaster {
        let block_signer = BlockSigner::new(keypair);
        Broadcaster {
            contexts: LruCache::new(NonZeroUsize::new(1000).unwrap()),
            quorum: BrachaQuorum::new(config),
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
            Echo(_) => {
                log::trace!("Processing ECHO {}", rb_msg.id);
                self.process_echo(rb_msg).await
            }
            Vote(_) => {
                log::trace!("Processing VOTE {}", rb_msg.id);
                self.process_vote(rb_msg).await
            }
            _ => Err(InvalidBroadcast(format!(
                "Invalid broadcast message {}",
                rb_msg.id
            ))),
        };
        log::trace!("Context after new broadcast: {:?}", self.contexts.get(&id));
        result
    }

    // on receiving <v> from leader:
    // if echo == true:
    // send <echo, v> to all parties
    // echo = false
    //
    // on receiving <echo, v> from n-f distinct parties:
    // if vote == true:
    // send <vote, v> to all parties
    // vote = false
    async fn process_echo(&mut self, rb_msg: RbMsg) -> Result<ProtocolResponse, BroadcastError> {
        let block = rb_msg.get_data().expect("Block should be present");
        let mut ctx = self.contexts.get_or_insert_mut(rb_msg.id.clone(), || {
            ConsensusContext::new(rb_msg.id.clone())
        });

        if self.peer_id != rb_msg.original_sender {
            ctx.add_echo(rb_msg.original_sender);
        }

        //FIXME:
        //Protocol signatures processing needs to be tighten up.
        //What kind of signature goes with each protocol message...
        //Do we trust underlying networking layer to deal with nodes identity authentication?
        let signature = self.block_signer.sign_block(block.clone())?;

        //if echo = true:
        if !ctx.echo.contains(&self.peer_id) {
            //echo = false
            ctx.add_echo(self.peer_id);

            //send <echo, v> to all parties
            return Ok(ProtocolResponse {
                status: Status::Pending,
                command: Command::Broadcast,
                protocol_reply: rb_msg.echo_reply(self.peer_id, block.clone(), signature),
            });
        }

        //on receiving <echo, v> from n-f distinct parties:
        //if vote == true:
        if !ctx.vote.contains(&self.peer_id)
            && self.quorum.check_threshold(ctx, rb_msg.phase.clone())
        {
            log::trace!("Prepare completed for {}", rb_msg.id);

            //vote = false
            ctx.echo_threshold = true;

            ctx.add_vote(self.peer_id);

            return Ok(ProtocolResponse {
                status: Status::Pending,
                command: Command::Broadcast,
                protocol_reply: rb_msg.vote_reply(self.peer_id, block.clone(), signature),
            });
        }

        Ok(ProtocolResponse {
            status: Status::Pending,
            command: Command::Drop,
            protocol_reply: rb_msg.ack_reply(self.peer_id, signature),
        })
    }

    // on receiving <vote, v> from f+1 distinct parties:
    // if vote == true:
    // send <vote, v> to all parties
    // vote = false
    //
    // on receiving <vote, v> from n-f distinct parties:
    // deliver v
    async fn process_vote(&mut self, rb_msg: RbMsg) -> Result<ProtocolResponse, BroadcastError> {
        let block = rb_msg.get_data().expect("Block should be present");
        let mut ctx = self.contexts.get_or_insert_mut(rb_msg.id.clone(), || {
            ConsensusContext::new(rb_msg.id.clone())
        });

        if self.peer_id != rb_msg.original_sender {
            ctx.add_vote(rb_msg.original_sender);
        }

        let signature = self.block_signer.sign_block(block.clone())?;

        //on receiving <vote, v> from f+1 distinct parties:
        //if vote == true:
        if !ctx.vote.contains(&self.peer_id)
            && self.quorum.check_threshold(ctx, rb_msg.phase.clone())
        {
            //vote = false
            ctx.add_vote(self.peer_id);

            return Ok(ProtocolResponse {
                status: Status::Pending,
                command: Command::Broadcast,
                protocol_reply: rb_msg.vote_reply(self.peer_id, block.clone(), signature),
            });
        }

        //on receiving <vote, v> from n-f distinct parties:
        //deliver v
        if !ctx.vote_threshold && self.quorum.check_threshold(ctx, rb_msg.phase.clone()) {
            log::debug!("Commit complete for {}", rb_msg.id);
            ctx.vote_threshold = true;

            return Ok(ProtocolResponse {
                status: Status::Committed,
                command: Command::Drop,
                protocol_reply: rb_msg.ack_reply(self.peer_id, signature),
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
