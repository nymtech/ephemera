use lru::LruCache;
use std::num::NonZeroUsize;

use crate::{
    block::types::block::Block,
    broadcast::{
        bracha::quorum::BrachaQuorum,
        Command,
        MessageType::{Echo, Vote},
        ProtocolContext, RawRbMsg, Status,
    },
    config::BroadcastConfig,
    network::peer::PeerId,
    utilities::hash::HashType,
};

pub(crate) struct Broadcaster {
    contexts: LruCache<HashType, ProtocolContext>,
    quorum: BrachaQuorum,
    local_peer_id: PeerId,
}

#[derive(Debug)]
pub(crate) struct ProtocolResponse {
    pub(crate) status: Status,
    pub(crate) command: Command,
    pub(crate) protocol_reply: Option<RawRbMsg>,
}

impl Broadcaster {
    pub fn new(config: BroadcastConfig, peer_id: PeerId) -> Broadcaster {
        Broadcaster {
            contexts: LruCache::new(NonZeroUsize::new(1000).unwrap()),
            quorum: BrachaQuorum::new(config),
            local_peer_id: peer_id,
        }
    }

    pub(crate) async fn new_broadcast(&mut self, block: Block) -> anyhow::Result<ProtocolResponse> {
        log::debug!("Starting protocol for new block {}", block);
        let rb_msg = RawRbMsg::new(block, self.local_peer_id);

        log::debug!("New broadcast message: {:?}", rb_msg.id);
        self.handle(rb_msg).await
    }

    pub(crate) async fn handle(&mut self, rb_msg: RawRbMsg) -> anyhow::Result<ProtocolResponse> {
        match rb_msg.message_type.clone() {
            Echo(_) => {
                log::trace!("Processing ECHO {:?}", rb_msg.id);
                self.process_echo(rb_msg).await
            }
            Vote(_) => {
                log::trace!("Processing VOTE {:?}", rb_msg.id);
                self.process_vote(rb_msg).await
            }
        }
    }

    pub(crate) async fn topology_updated(&mut self, peers: Vec<PeerId>) -> anyhow::Result<()> {
        self.quorum.update_topology(peers.len());
        Ok(())
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
    async fn process_echo(&mut self, rb_msg: RawRbMsg) -> anyhow::Result<ProtocolResponse> {
        let block = rb_msg.get_block();
        let hash = block.hash_with_default_hasher()?;
        let ctx = self
            .contexts
            .get_or_insert_mut(hash, || ProtocolContext::new(hash, self.local_peer_id));

        if self.local_peer_id != rb_msg.original_sender {
            ctx.add_echo(rb_msg.original_sender);
        }

        //if echo = true:
        if !ctx.echoed() {
            //echo = false
            ctx.add_echo(self.local_peer_id);

            //send <echo, v> to all parties
            return Ok(ProtocolResponse {
                status: Status::Pending,
                command: Command::Broadcast,
                protocol_reply: Some(rb_msg.echo_reply(self.local_peer_id, block.clone())),
            });
        }

        //on receiving <echo, v> from n-f distinct parties:
        //if vote == true:
        if !ctx.voted()
            && self
                .quorum
                .check_threshold(ctx, rb_msg.message_type.clone().into())
                .is_vote()
        {
            log::trace!("Prepare completed for {:?}", rb_msg.id);

            ctx.add_vote(self.local_peer_id);

            return Ok(ProtocolResponse {
                status: Status::Pending,
                command: Command::Broadcast,
                protocol_reply: Some(rb_msg.vote_reply(self.local_peer_id, block.clone())),
            });
        }

        Ok(ProtocolResponse {
            status: Status::Pending,
            command: Command::Drop,
            protocol_reply: None,
        })
    }

    // on receiving <vote, v> from f+1 distinct parties:
    // if vote == true:
    // send <vote, v> to all parties
    // vote = false
    //
    // on receiving <vote, v> from n-f distinct parties:
    // deliver v
    async fn process_vote(&mut self, rb_msg: RawRbMsg) -> anyhow::Result<ProtocolResponse> {
        let block = rb_msg.get_block();
        let hash = block.hash_with_default_hasher()?;
        let ctx = self
            .contexts
            .get_or_insert_mut(hash, || ProtocolContext::new(hash, self.local_peer_id));

        if self.local_peer_id != rb_msg.original_sender {
            ctx.add_vote(rb_msg.original_sender);
        }

        //on receiving <vote, v> from f+1 distinct parties:
        //if vote == true:
        if self
            .quorum
            .check_threshold(ctx, rb_msg.message_type.clone().into())
            .is_vote()
        {
            //vote = false
            ctx.add_vote(self.local_peer_id);

            return Ok(ProtocolResponse {
                status: Status::Pending,
                command: Command::Broadcast,
                protocol_reply: Some(rb_msg.vote_reply(self.local_peer_id, block.clone())),
            });
        }

        //on receiving <vote, v> from n-f distinct parties:
        //deliver v
        if self
            .quorum
            .check_threshold(ctx, rb_msg.message_type.clone().into())
            .is_deliver()
        {
            log::debug!("Commit complete for {:?}", rb_msg.id);

            return Ok(ProtocolResponse {
                status: Status::Completed,
                command: Command::Drop,
                protocol_reply: None,
            });
        }

        Ok(ProtocolResponse {
            status: Status::Pending,
            command: Command::Drop,
            protocol_reply: None,
        })
    }
}

#[cfg(test)]
mod tests {

    //1.make sure before voting enough echo messages are received
    //2.make sure before delivering enough vote messages are received
    //a)Either f + 1
    //b)Or n - f

    //3.make sure that duplicate messages doesn't have impact

    //4. "Ideally" make sure that when topology changes, the ongoing broadcast can deal with it

    use assert_matches::assert_matches;

    use crate::broadcast::bracha::broadcaster::ProtocolResponse;
    use crate::utilities::hash::HashType;
    use crate::{
        block::types::block::{Block, RawBlock, RawBlockHeader},
        broadcast::{self, bracha::broadcaster::Broadcaster, RawRbMsg},
        config::BroadcastConfig,
        network::peer::PeerId,
    };

    #[tokio::test]
    async fn test_state_transitions_from_start_to_end() {
        let local_peer_id = PeerId::random();
        let block_creator_peer_id = PeerId::random();

        let mut broadcaster = Broadcaster::new(BroadcastConfig { cluster_size: 10 }, local_peer_id);

        let (block_hash, block) = create_block(block_creator_peer_id);

        receive_echo_first_message(&mut broadcaster, &block).await;

        let ctx = broadcaster.contexts.get(&block_hash).unwrap();
        assert_eq!(ctx.echo.len(), 2);
        assert!(ctx.echoed());
        assert!(!ctx.voted());

        receive_nr_of_echo_messages_below_vote_threshold(6, &mut broadcaster, &block).await;

        let ctx = broadcaster.contexts.get(&block_hash).unwrap();
        assert_eq!(ctx.echo.len(), 7);
        assert!(ctx.echoed());
        assert!(!ctx.voted());

        receive_echo_threshold_message(&mut broadcaster, &block).await;

        let ctx = broadcaster.contexts.get(&block_hash).unwrap();
        assert_eq!(ctx.echo.len(), 8);
        assert_eq!(ctx.vote.len(), 1);
        assert!(ctx.echoed());
        assert!(ctx.voted());

        receive_nr_of_vote_messages_below_deliver_threshold(6, &mut broadcaster, &block).await;

        let ctx = broadcaster.contexts.get(&block_hash).unwrap();
        assert_eq!(ctx.echo.len(), 8);
        assert_eq!(ctx.vote.len(), 7);
        assert!(ctx.echoed());
        assert!(ctx.voted());

        receive_threshold_vote_message_for_deliver(&mut broadcaster, &block).await;
    }

    async fn receive_threshold_vote_message_for_deliver(
        broadcaster: &mut Broadcaster,
        block: &Block,
    ) {
        let rb_msg = RawRbMsg::new(block.clone(), PeerId::random());
        let rb_msg = rb_msg.vote_reply(PeerId::random(), block.clone());

        let response = handle_double(broadcaster, rb_msg).await;

        assert_matches!(response.status, broadcast::Status::Completed);
        assert_matches!(response.command, broadcast::Command::Drop);
        assert_matches!(response.protocol_reply, None);
    }

    async fn receive_nr_of_echo_messages_below_vote_threshold(
        nr: usize,
        broadcaster: &mut Broadcaster,
        block: &Block,
    ) {
        for _ in 1..nr {
            let rb_msg = RawRbMsg::new(block.clone(), PeerId::random());

            let response = handle_double(broadcaster, rb_msg).await;

            assert_matches!(response.status, broadcast::Status::Pending);
            assert_matches!(response.command, broadcast::Command::Drop);
            assert_matches!(response.protocol_reply, None);
        }
    }

    async fn receive_nr_of_vote_messages_below_deliver_threshold(
        nr: usize,
        broadcaster: &mut Broadcaster,
        block: &Block,
    ) {
        for _ in 0..nr {
            let rb_msg = RawRbMsg::new(block.clone(), PeerId::random());
            let rb_msg = rb_msg.vote_reply(PeerId::random(), block.clone());

            let response = handle_double(broadcaster, rb_msg).await;
            assert_matches!(response.status, broadcast::Status::Pending);
            assert_matches!(response.command, broadcast::Command::Drop);
            assert_matches!(response.protocol_reply, None);
        }
    }

    async fn receive_echo_first_message(broadcaster: &mut Broadcaster, block: &Block) {
        let message_sender_peer_id = PeerId::random();
        let rb_msg = RawRbMsg::new(block.clone(), message_sender_peer_id);

        let response = handle_double(broadcaster, rb_msg).await;
        assert_matches!(response.status, broadcast::Status::Pending);
        assert_matches!(response.command, broadcast::Command::Broadcast);
        assert_matches!(
            response.protocol_reply,
            Some(RawRbMsg {
                id: _,
                request_id: _,
                original_sender: _,
                timestamp: _,
                message_type: broadcast::MessageType::Echo(_),
            })
        );
    }

    async fn receive_echo_threshold_message(broadcaster: &mut Broadcaster, block: &Block) {
        let message_sender_peer_id = PeerId::random();
        let rb_msg = RawRbMsg::new(block.clone(), message_sender_peer_id);

        let response = handle_double(broadcaster, rb_msg).await;
        assert_matches!(response.status, broadcast::Status::Pending);
        assert_matches!(response.command, broadcast::Command::Broadcast);
        assert_matches!(
            response.protocol_reply,
            Some(RawRbMsg {
                id: _,
                request_id: _,
                original_sender: _,
                timestamp: _,
                message_type: broadcast::MessageType::Vote(_),
            })
        );
    }

    fn create_block(block_creator_peer_id: PeerId) -> (HashType, Block) {
        let header = RawBlockHeader::new(block_creator_peer_id, 0);
        let raw_block = RawBlock::new(header.clone(), vec![]);
        let block_hash = raw_block.hash_with_default_hasher().unwrap();
        let block = Block::new(raw_block, block_hash);
        (block_hash, block)
    }

    //make sure that duplicate messages doesn't have impact
    async fn handle_double(broadcaster: &mut Broadcaster, rb_msg: RawRbMsg) -> ProtocolResponse {
        let response = broadcaster.handle(rb_msg.clone()).await.unwrap();
        broadcaster.handle(rb_msg).await.unwrap();
        response
    }
}
