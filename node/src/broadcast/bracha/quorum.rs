use crate::broadcast::{MessageType, ProtocolContext};
use log::{info, trace};

pub(crate) struct BrachaQuorum {
    pub(crate) cluster_size: usize,
    pub(crate) max_faulty_nodes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BrachaMessageType {
    Echo,
    Vote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BrachaAction {
    Vote,
    Deliver,
    Ignore,
}

impl BrachaAction {
    pub(crate) fn is_vote(&self) -> bool {
        matches!(self, BrachaAction::Vote)
    }

    pub(crate) fn is_deliver(&self) -> bool {
        matches!(self, BrachaAction::Deliver)
    }
}

impl From<MessageType> for BrachaMessageType {
    fn from(message_type: MessageType) -> Self {
        match message_type {
            MessageType::Echo(_) => BrachaMessageType::Echo,
            MessageType::Vote(_) => BrachaMessageType::Vote,
        }
    }
}

const MAX_FAULTY_RATIO: f64 = 1.0 / 3.0;

impl BrachaQuorum {
    pub fn new() -> Self {
        Self {
            cluster_size: 0,
            max_faulty_nodes: 0,
        }
    }

    pub(crate) fn update_group_size(&mut self, size: usize) {
        //As we don't have strong guarantees/consensus/timing constraints on the
        //broadcast, we just update group immediately.
        //Theoretically it can break existing ongoing broadcast but timing chances for it
        //probably are very low.
        self.cluster_size = size;
        self.max_faulty_nodes = (self.cluster_size as f64 * MAX_FAULTY_RATIO).floor() as usize;
        info!(
            "Bracha quorum: cluster_size: {}, max_faulty_nodes: {}",
            self.cluster_size, self.max_faulty_nodes
        );
    }

    pub(crate) fn check_threshold(
        &self,
        ctx: &ProtocolContext,
        phase: BrachaMessageType,
    ) -> BrachaAction {
        if self.cluster_size == 0 {
            return BrachaAction::Ignore;
        }

        match phase {
            BrachaMessageType::Echo => {
                if ctx.echo.len() >= self.cluster_size - self.max_faulty_nodes {
                    trace!(
                        "Echo threshold reached: Echoed:{} / Threshold:{} for Block:{}",
                        ctx.echo.len(),
                        self.cluster_size - self.max_faulty_nodes,
                        ctx.hash
                    );
                    BrachaAction::Vote
                } else {
                    trace!(
                        "Echo threshold not reached: Echoed:{} / Threshold:{} for Block:{}",
                        ctx.echo.len(),
                        self.cluster_size - self.max_faulty_nodes,
                        ctx.hash
                    );
                    BrachaAction::Ignore
                }
            }
            BrachaMessageType::Vote => {
                if !ctx.voted() {
                    // f + 1 votes are enough to send our vote
                    if ctx.vote.len() >= self.max_faulty_nodes {
                        trace!(
                            "Vote send threshold reached: Voted:{} / Threshold:{} for Block:{}",
                            ctx.vote.len(),
                            self.max_faulty_nodes + 1,
                            ctx.hash
                        );
                        return BrachaAction::Vote;
                    }
                }

                if ctx.voted() {
                    // n-f votes are enough to deliver the value
                    if ctx.vote.len() >= self.cluster_size - self.max_faulty_nodes {
                        trace!(
                            "Deliver threshold reached: Voted:{} / Threshold:{} for Block:{}",
                            ctx.vote.len(),
                            self.cluster_size - self.max_faulty_nodes,
                            ctx.hash
                        );
                        return BrachaAction::Deliver;
                    }
                }

                trace!(
                    "Vote threshold not reached: Voted:{} / Threshold:{} for Block:{}",
                    ctx.vote.len(),
                    self.max_faulty_nodes + 1,
                    ctx.hash
                );
                BrachaAction::Ignore
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::broadcast::{
        bracha::quorum::{BrachaAction, BrachaMessageType, BrachaQuorum},
        ProtocolContext,
    };
    use crate::peer::PeerId;

    #[test]
    fn test_max_faulty_nodes() {
        let mut quorum = BrachaQuorum::new();
        quorum.update_group_size(10);
        assert_eq!(quorum.max_faulty_nodes, 3);
    }

    #[test]
    fn test_vote_threshold_from_n_minus_f_peers() {
        let mut quorum = BrachaQuorum::new();
        quorum.update_group_size(10);

        let ctx = ctx_with_nr_echoes(0);
        assert_eq!(
            quorum.check_threshold(&ctx, BrachaMessageType::Echo),
            BrachaAction::Ignore
        );

        let ctx = ctx_with_nr_echoes(3);
        assert_eq!(
            quorum.check_threshold(&ctx, BrachaMessageType::Echo),
            BrachaAction::Ignore
        );

        let ctx = ctx_with_nr_echoes(8);
        assert_eq!(
            quorum.check_threshold(&ctx, BrachaMessageType::Echo),
            BrachaAction::Vote
        );
    }

    #[test]
    fn test_vote_threshold_from_f_plus_one_peers() {
        let mut quorum = BrachaQuorum::new();
        quorum.update_group_size(10);

        let ctx = ctx_with_nr_votes(0, None);
        assert_eq!(
            quorum.check_threshold(&ctx, BrachaMessageType::Vote),
            BrachaAction::Ignore
        );

        let ctx = ctx_with_nr_votes(2, None);
        assert_eq!(
            quorum.check_threshold(&ctx, BrachaMessageType::Vote),
            BrachaAction::Ignore
        );

        let ctx = ctx_with_nr_votes(5, None);
        assert_eq!(
            quorum.check_threshold(&ctx, BrachaMessageType::Vote),
            BrachaAction::Vote
        );
    }

    #[test]
    fn test_deliver_threshold_from_n_minus_f_peers() {
        let mut quorum = BrachaQuorum::new();
        quorum.update_group_size(10);

        let local_peer_id = PeerId::random();
        let ctx = ctx_with_nr_votes(0, local_peer_id.into());
        assert_eq!(
            quorum.check_threshold(&ctx, BrachaMessageType::Vote),
            BrachaAction::Ignore
        );

        let ctx = ctx_with_nr_votes(3, local_peer_id.into());
        assert_eq!(
            quorum.check_threshold(&ctx, BrachaMessageType::Vote),
            BrachaAction::Ignore
        );

        let ctx = ctx_with_nr_votes(7, local_peer_id.into());
        assert_eq!(
            quorum.check_threshold(&ctx, BrachaMessageType::Vote),
            BrachaAction::Deliver
        );
    }

    #[test]
    fn test_change_group() {
        let mut quorum = BrachaQuorum::new();
        quorum.update_group_size(10);
        assert_eq!(quorum.max_faulty_nodes, 3);

        quorum.update_group_size(13);
        assert_eq!(quorum.max_faulty_nodes, 4);
    }

    fn ctx_with_nr_echoes(n: usize) -> ProtocolContext {
        let mut ctx = ProtocolContext {
            local_peer_id: PeerId::random(),
            hash: [0; 32].into(),
            echo: Default::default(),
            vote: Default::default(),
            delivered: false,
        };
        for _ in 0..n {
            ctx.echo.insert(PeerId::random());
        }
        ctx
    }

    fn ctx_with_nr_votes(n: usize, local_peer_id: Option<PeerId>) -> ProtocolContext {
        let mut ctx = ProtocolContext {
            local_peer_id: local_peer_id.unwrap_or(PeerId::random()),
            hash: [0; 32].into(),
            echo: Default::default(),
            vote: Default::default(),
            delivered: false,
        };
        for _ in 0..n {
            ctx.vote.insert(PeerId::random());
        }
        if let Some(id) = local_peer_id {
            ctx.vote.insert(id);
        }
        ctx
    }
}
