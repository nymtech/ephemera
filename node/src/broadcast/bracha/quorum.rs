use crate::broadcast::{MessageType, ProtocolContext};

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

    /// Notify quota about topology change.
    pub(crate) fn update_topology(&mut self, nr_of_peers: usize) {
        //As we don't have strong guarantees/consensus/timing constraints on the
        //broadcast, we just update topology immediately.
        //Theoretically it can break existing ongoing broadcast but timing chances for it
        //probably are very low.
        self.cluster_size = nr_of_peers;
        self.max_faulty_nodes = (self.cluster_size as f64 * MAX_FAULTY_RATIO).floor() as usize;
        log::info!(
            "Bracha quorum: cluster_size: {}, max_faulty_nodes: {}",
            self.cluster_size,
            self.max_faulty_nodes
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
                if ctx.echo.len() > self.cluster_size - self.max_faulty_nodes {
                    log::debug!(
                        "Echo threshold reached: {}/{} for {}",
                        ctx.echo.len(),
                        self.cluster_size - self.max_faulty_nodes,
                        ctx.hash
                    );
                    BrachaAction::Vote
                } else {
                    BrachaAction::Ignore
                }
            }
            BrachaMessageType::Vote => {
                if !ctx.voted() {
                    // f + 1 votes are enough to send our vote
                    if ctx.vote.len() > self.max_faulty_nodes {
                        log::debug!(
                            "Vote send threshold reached: {}/{} for {}",
                            ctx.vote.len(),
                            self.max_faulty_nodes + 1,
                            ctx.hash
                        );
                        return BrachaAction::Vote;
                    }
                }

                if ctx.voted() {
                    // n-f votes are enough to deliver the value
                    if ctx.vote.len() > self.cluster_size - self.max_faulty_nodes {
                        log::debug!(
                            "Deliver threshold reached: {}/{} for {}",
                            ctx.vote.len(),
                            self.cluster_size - self.max_faulty_nodes,
                            ctx.hash
                        );
                        return BrachaAction::Deliver;
                    }
                }

                BrachaAction::Ignore
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        broadcast::{
            bracha::quorum::{BrachaAction, BrachaMessageType, BrachaQuorum},
            ProtocolContext,
        },
        network::peer::PeerId,
    };

    #[test]
    fn test_max_faulty_nodes() {
        let mut quorum = BrachaQuorum::new();
        quorum.update_topology(10);
        assert_eq!(quorum.max_faulty_nodes, 3);
    }

    #[test]
    fn test_vote_threshold_from_n_minus_f_peers() {
        let mut quorum = BrachaQuorum::new();
        quorum.update_topology(10);

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
        quorum.update_topology(10);

        let ctx = ctx_with_nr_votes(0, None);
        assert_eq!(
            quorum.check_threshold(&ctx, BrachaMessageType::Vote),
            BrachaAction::Ignore
        );

        let ctx = ctx_with_nr_votes(3, None);
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
        quorum.update_topology(10);

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
    fn test_change_topology() {
        let mut quorum = BrachaQuorum::new();
        quorum.update_topology(10);
        assert_eq!(quorum.max_faulty_nodes, 3);

        quorum.update_topology(13);
        assert_eq!(quorum.max_faulty_nodes, 4);
    }

    fn ctx_with_nr_echoes(n: usize) -> ProtocolContext {
        let mut ctx = ProtocolContext {
            local_peer_id: PeerId::random(),
            hash: [0; 32].into(),
            echo: Default::default(),
            vote: Default::default(),
            topology_id: 0,
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
            topology_id: 0,
        };
        for _ in 0..n {
            ctx.vote.insert(PeerId::random());
        }
        if local_peer_id.is_some() {
            ctx.vote.insert(local_peer_id.unwrap());
        }
        ctx
    }
}
