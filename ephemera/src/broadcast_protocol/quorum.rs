use crate::settings::QuorumSettings;

pub trait Quorum {
    fn prepare_threshold(&self, ready: u64) -> bool;
    fn commit_threshold(&self, ready: u64) -> bool;
}

#[derive(Debug, Clone)]
pub struct BasicQuorum {
    pub size: u64,
    pub threshold: u64,
}

impl BasicQuorum {
    pub fn new(settings: QuorumSettings) -> Self {
        BasicQuorum {
            size: settings.total,
            threshold: settings.threshold,
        }
    }
}

impl Quorum for BasicQuorum {
    fn prepare_threshold(&self, ready: u64) -> bool {
        ready == self.size
    }

    fn commit_threshold(&self, ready: u64) -> bool {
        ready == self.size
    }
}
