use crate::config::BroadcastProtocolSettings;

pub trait Quorum {
    fn prepare_threshold(&self, ready: usize) -> bool;
    fn commit_threshold(&self, ready: usize) -> bool;
}

#[derive(Debug, Clone)]
pub struct BasicQuorum {
    pub size: usize,
    pub threshold: usize,
}

impl BasicQuorum {
    pub fn new(settings: BroadcastProtocolSettings) -> Self {
        BasicQuorum {
            size: settings.cluster_size,
            threshold: settings.quorum_threshold_size,
        }
    }
}

impl Quorum for BasicQuorum {
    fn prepare_threshold(&self, ready: usize) -> bool {
        ready == self.size
    }

    fn commit_threshold(&self, ready: usize) -> bool {
        ready == self.size
    }
}
