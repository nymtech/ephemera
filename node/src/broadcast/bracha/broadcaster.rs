use std::num::NonZeroUsize;

use lru::LruCache;

use crate::{
    block::types::block::Block,
    broadcast::{
        bracha::{quorum::BrachaQuorum, topology::BroadcastTopology},
        Command,
        MessageType::{Echo, Vote},
        ProtocolContext, RawRbMsg, Status,
    },
    network::peer::PeerId,
    utilities::hash::HashType,
};

#[derive(Debug)]
pub(crate) struct ProtocolResponse {
    pub(crate) status: Status,
    pub(crate) command: Command,
    pub(crate) protocol_reply: Option<RawRbMsg>,
}

impl ProtocolResponse {
    pub(crate) fn broadcast(msg: RawRbMsg) -> ProtocolResponse {
        ProtocolResponse {
            status: Status::Pending,
            command: Command::Broadcast,
            protocol_reply: Some(msg),
        }
    }

    pub(crate) fn deliver() -> ProtocolResponse {
        ProtocolResponse {
            status: Status::Completed,
            command: Command::Drop,
            protocol_reply: None,
        }
    }

    pub(crate) fn drop() -> ProtocolResponse {
        ProtocolResponse {
            status: Status::Pending,
            command: Command::Drop,
            protocol_reply: None,
        }
    }
}

pub(crate) struct Broadcaster {
    contexts: LruCache<HashType, ProtocolContext>,
    quorum: BrachaQuorum,
    local_peer_id: PeerId,
    topology: BroadcastTopology,
}

impl Broadcaster {
    pub fn new(peer_id: PeerId) -> Broadcaster {
        Broadcaster {
            contexts: LruCache::new(NonZeroUsize::new(1000).unwrap()),
            quorum: BrachaQuorum::new(),
            local_peer_id: peer_id,
            topology: BroadcastTopology::new(),
        }
    }

    pub(crate) async fn new_broadcast(&mut self, block: Block) -> anyhow::Result<ProtocolResponse> {
        log::debug!("Starting broadcast for new block {}", block);
        let rb_msg = RawRbMsg::new(block, self.local_peer_id);
        self.handle(rb_msg).await
    }

    pub(crate) async fn handle(&mut self, rb_msg: RawRbMsg) -> anyhow::Result<ProtocolResponse> {
        log::debug!("New broadcast message: {:?}", rb_msg.id);

        let block = rb_msg.block_ref();
        let hash = block.hash_with_default_hasher()?;

        if let Some(err_resp) = self.check_topology(hash, &rb_msg) {
            return err_resp;
        }

        if !self.contexts.contains(&hash) {
            self.contexts.put(
                hash,
                ProtocolContext::new(hash, self.local_peer_id, self.topology.current_id),
            );
        }

        match rb_msg.message_type.clone() {
            Echo(_) => {
                log::trace!("Processing ECHO {:?}", rb_msg.id);
                self.process_echo(rb_msg, hash).await
            }
            Vote(_) => {
                log::trace!("Processing VOTE {:?}", rb_msg.id);
                self.process_vote(rb_msg, hash).await
            }
        }
    }

    pub(crate) fn topology_updated(&mut self, peers: Vec<PeerId>) {
        log::info!("Topology updated: {:?}", peers);
        self.quorum.update_topology(peers.len());
        self.topology.add_snapshot(peers);
    }

    fn check_topology(
        &mut self,
        hash: HashType,
        rb_msg: &RawRbMsg,
    ) -> Option<anyhow::Result<ProtocolResponse>> {
        if !self.contexts.contains(&hash) {
            //This can happen at startup for example when node is not ready yet(caught up with the network)
            if self.topology.is_empty() {
                log::warn!(
                    "Received message for block {:?} but topology is empty",
                    hash
                );
                return Ok(ProtocolResponse::drop()).into();
            }

            //Node is excluded from topology for some reason(for example health checks failed)
            if !self
                .topology
                .is_member(self.topology.current_id, &self.local_peer_id)
            {
                log::warn!(
                    "Received message for block {} but current node {} is not part of the current topology",
                    hash, self.local_peer_id
                );
                return Ok(ProtocolResponse::drop()).into();
            }
        }

        //Make sure that the sender peer_id and block peer_id are part of the block initial topology
        //1. If the block is new, the topology is the current one
        //2. If the block is old, the topology is the one that was used when the block was created

        //It's needed to make sure that
        //1. The peer is authenticated(part of the network)
        //2. Block processing is consistent regarding the membership across rounds

        let topology_id = self
            .contexts
            .get(&hash)
            .map(|c| c.topology_id)
            .unwrap_or_else(|| self.topology.current_id);

        let block_creator_in_topology = self
            .topology
            .is_member(topology_id, &rb_msg.block_ref().header.creator);

        if !block_creator_in_topology {
            log::trace!("Received block {:?} what is created by peer {:?} who is not part of the current topology",
                hash, rb_msg.block_ref().header.creator);

            return Ok(ProtocolResponse::drop()).into();
        }

        let sender_in_topology = self
            .topology
            .is_member(topology_id, &rb_msg.original_sender);

        if !sender_in_topology {
            log::trace!("Received message from peer {:?} who is not part of current topology for block {:?}",
                rb_msg.original_sender, hash);

            return Ok(ProtocolResponse::drop()).into();
        }
        None
    }

    async fn process_echo(
        &mut self,
        rb_msg: RawRbMsg,
        hash: HashType,
    ) -> anyhow::Result<ProtocolResponse> {
        let ctx = self.contexts.get_mut(&hash).unwrap();

        if self.local_peer_id != rb_msg.original_sender {
            ctx.add_echo(rb_msg.original_sender);
        }

        if !ctx.echoed() {
            ctx.add_echo(self.local_peer_id);
            return Ok(ProtocolResponse::broadcast(
                rb_msg.echo_reply(self.local_peer_id, rb_msg.block()),
            ));
        }

        if !ctx.voted()
            && self
                .quorum
                .check_threshold(ctx, rb_msg.message_type.clone().into())
                .is_vote()
        {
            log::trace!("Prepare completed for {:?}", rb_msg.id);

            ctx.add_vote(self.local_peer_id);

            return Ok(ProtocolResponse::broadcast(
                rb_msg.vote_reply(self.local_peer_id, rb_msg.block()),
            ));
        }

        Ok(ProtocolResponse::drop())
    }

    async fn process_vote(
        &mut self,
        rb_msg: RawRbMsg,
        hash: HashType,
    ) -> anyhow::Result<ProtocolResponse> {
        let block = rb_msg.block_ref();
        let ctx = self.contexts.get_mut(&hash).unwrap();

        if self.local_peer_id != rb_msg.original_sender {
            ctx.add_vote(rb_msg.original_sender);
        }

        if self
            .quorum
            .check_threshold(ctx, rb_msg.message_type.clone().into())
            .is_vote()
        {
            ctx.add_vote(self.local_peer_id);

            return Ok(ProtocolResponse::broadcast(
                rb_msg.vote_reply(self.local_peer_id, block.clone()),
            ));
        }

        if self
            .quorum
            .check_threshold(ctx, rb_msg.message_type.clone().into())
            .is_deliver()
        {
            log::debug!("Commit complete for {:?}", rb_msg.id);

            return Ok(ProtocolResponse::deliver());
        }

        Ok(ProtocolResponse::drop())
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

    use std::iter;

    use assert_matches::assert_matches;

    use crate::broadcast::bracha::broadcaster::ProtocolResponse;
    use crate::utilities::hash::HashType;
    use crate::{
        block::types::block::{Block, RawBlock, RawBlockHeader},
        broadcast::{self, bracha::broadcaster::Broadcaster, RawRbMsg},
        network::peer::PeerId,
    };

    #[tokio::test]
    async fn test_state_transitions_from_start_to_end() {
        let peers: Vec<PeerId> = iter::repeat_with(PeerId::random).take(10).collect();
        let local_peer_id = peers[0];
        let block_creator_peer_id = peers[1];

        let mut broadcaster = Broadcaster::new(local_peer_id);
        broadcaster.topology_updated(peers.clone());

        let (block_hash, block) = create_block(block_creator_peer_id);

        //After this echo set contains local and block creator(msg sender)
        receive_echo_first_message(&mut broadcaster, &block, block_creator_peer_id).await;

        let ctx = broadcaster.contexts.get(&block_hash).unwrap();
        assert_eq!(ctx.echo.len(), 2);
        assert!(ctx.echoed());
        assert!(!ctx.voted());

        receive_nr_of_echo_messages_below_vote_threshold(&mut broadcaster, &block, &peers[2..7])
            .await;

        let ctx = broadcaster.contexts.get(&block_hash).unwrap();
        assert_eq!(ctx.echo.len(), 7);
        assert!(ctx.echoed());
        assert!(!ctx.voted());

        receive_echo_threshold_message(
            &mut broadcaster,
            &block,
            peers.iter().nth(7).unwrap().clone(),
        )
        .await;

        let ctx = broadcaster.contexts.get(&block_hash).unwrap();
        assert_eq!(ctx.echo.len(), 8);
        assert_eq!(ctx.vote.len(), 1);
        assert!(ctx.echoed());
        assert!(ctx.voted());

        receive_nr_of_vote_messages_below_deliver_threshold(&mut broadcaster, &block, &peers[2..8])
            .await;

        let ctx = broadcaster.contexts.get(&block_hash).unwrap();
        assert_eq!(ctx.echo.len(), 8);
        assert_eq!(ctx.vote.len(), 7);
        assert!(ctx.echoed());
        assert!(ctx.voted());

        receive_threshold_vote_message_for_deliver(
            &mut broadcaster,
            &block,
            peers.iter().nth(8).unwrap().clone(),
        )
        .await;
    }

    async fn receive_threshold_vote_message_for_deliver(
        broadcaster: &mut Broadcaster,
        block: &Block,
        peer_id: PeerId,
    ) {
        let rb_msg = RawRbMsg::new(block.clone(), PeerId::random());
        let rb_msg = rb_msg.vote_reply(peer_id, block.clone());

        let response = handle_double(broadcaster, rb_msg).await;

        assert_matches!(response.status, broadcast::Status::Completed);
        assert_matches!(response.command, broadcast::Command::Drop);
        assert_matches!(response.protocol_reply, None);
    }

    async fn receive_nr_of_echo_messages_below_vote_threshold(
        broadcaster: &mut Broadcaster,
        block: &Block,
        peers: &[PeerId],
    ) {
        for peer_id in peers {
            let rb_msg = RawRbMsg::new(block.clone(), peer_id.clone());

            let response = handle_double(broadcaster, rb_msg).await;

            assert_matches!(response.status, broadcast::Status::Pending);
            assert_matches!(response.command, broadcast::Command::Drop);
            assert_matches!(response.protocol_reply, None);
        }
    }

    async fn receive_nr_of_vote_messages_below_deliver_threshold(
        broadcaster: &mut Broadcaster,
        block: &Block,
        peers: &[PeerId],
    ) {
        for peer_id in peers {
            let rb_msg = RawRbMsg::new(block.clone(), PeerId::random());
            let rb_msg = rb_msg.vote_reply(peer_id.clone(), block.clone());

            let response = handle_double(broadcaster, rb_msg).await;
            assert_matches!(response.status, broadcast::Status::Pending);
            assert_matches!(response.command, broadcast::Command::Drop);
            assert_matches!(response.protocol_reply, None);
        }
    }

    async fn receive_echo_first_message(
        broadcaster: &mut Broadcaster,
        block: &Block,
        block_creator: PeerId,
    ) {
        let rb_msg = RawRbMsg::new(block.clone(), block_creator);
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

    async fn receive_echo_threshold_message(
        broadcaster: &mut Broadcaster,
        block: &Block,
        peer_id: PeerId,
    ) {
        let rb_msg = RawRbMsg::new(block.clone(), peer_id);

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
