use crate::broadcast::{ConsensusContext, MessageType, Quorum};
use crate::config::BroadcastConfig;

#[derive(Debug, Clone)]
pub struct BasicQuorum {
    pub size: usize,
    pub threshold: usize,
}

impl BasicQuorum {
    pub fn new(settings: BroadcastConfig) -> Self {
        BasicQuorum {
            size: settings.cluster_size,
            threshold: settings.cluster_size,
        }
    }
}

impl Quorum for BasicQuorum {
    fn check_threshold(&self, ctx: &ConsensusContext, phase: MessageType) -> bool {
        match phase {
            MessageType::Echo(_) => ctx.echo.len() >= self.threshold,
            MessageType::Vote(_) => ctx.vote.len() >= self.threshold,
            _ => {
                //FIXME: Ack doesn't make sense
                false
            }
        }
    }
}
