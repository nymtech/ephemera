pub trait Quorum {
    fn prepare_threshold(&self, ready: usize) -> bool;
    fn commit_threshold(&self, ready: usize) -> bool;
}

#[derive(Debug, Clone)]
pub struct BasicQuorum {
    pub size: usize,
}

impl BasicQuorum {
    pub fn new(size: usize) -> Self {
        BasicQuorum { size }
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
