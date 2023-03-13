use crate::broadcast::{ConsensusContext, MessageType, Quorum};
use crate::config::BroadcastConfig;

pub(crate) struct BrachaQuorum {
    pub(crate) cluster_size: usize,
    pub(crate) max_faulty_nodes: usize,
}

const MAX_FAULTY_RATIO: f64 = 1.0 / 3.0;

impl BrachaQuorum {
    pub fn new(settings: BroadcastConfig) -> Self {
        let max_faulty_nodes = (settings.cluster_size as f64 * MAX_FAULTY_RATIO).ceil() as usize;
        log::info!(
            "Bracha quorum: cluster_size: {}, max_faulty_nodes: {}",
            settings.cluster_size,
            max_faulty_nodes
        );
        BrachaQuorum {
            cluster_size: settings.cluster_size,
            max_faulty_nodes,
        }
    }
}

impl Quorum for BrachaQuorum {
    fn check_threshold(&self, ctx: &ConsensusContext, phase: MessageType) -> bool {
        match phase {
            MessageType::Echo(_) => {
                if ctx.echo.len() >= self.cluster_size - self.max_faulty_nodes {
                    log::debug!(
                        "Echo threshold reached: {}/{} for {}",
                        ctx.echo.len(),
                        self.cluster_size - self.max_faulty_nodes,
                        ctx.id
                    );
                    true
                } else {
                    false
                }
            }
            MessageType::Vote(_) => {
                let votes_threshold_to_send_our_vote = ctx.vote.len() > self.max_faulty_nodes;
                if votes_threshold_to_send_our_vote {
                    log::debug!(
                        "Vote send threshold reached: {}/{} for {}",
                        ctx.vote.len(),
                        self.max_faulty_nodes + 1,
                        ctx.id
                    );
                }
                let votes_threshold_to_deliver =
                    ctx.vote.len() >= self.cluster_size - self.max_faulty_nodes;
                if votes_threshold_to_deliver {
                    log::debug!(
                        "Deliver threshold reached: {}/{} for {}",
                        ctx.vote.len(),
                        self.cluster_size - self.max_faulty_nodes,
                        ctx.id
                    );
                }
                votes_threshold_to_send_our_vote || votes_threshold_to_deliver
            }
        }
    }
}
