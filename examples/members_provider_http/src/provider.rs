use ephemera::membership::JsonPeerInfo;

#[derive(Clone)]
pub(crate) struct PeersProvider {
    peers: Vec<JsonPeerInfo>,
}

impl PeersProvider {
    pub(crate) fn new(peers: Vec<JsonPeerInfo>) -> Self {
        Self { peers }
    }

    pub(crate) fn peers(&self) -> Vec<JsonPeerInfo> {
        self.peers.clone()
    }
}

#[derive(Clone)]
pub(crate) struct ReducingPeerProvider {
    peers: Vec<JsonPeerInfo>,
    //For example if peers length is 6 and ratio is 0.2, then 1 peer will be removed.
    reduce_ratio: f64,
}

impl ReducingPeerProvider {
    pub(crate) fn new(peers: Vec<JsonPeerInfo>, reduce_ratio: f64) -> Self {
        Self {
            peers,
            reduce_ratio,
        }
    }

    pub(crate) fn peers(&self) -> Vec<JsonPeerInfo> {
        let mut reduced_peers = vec![];
        let reduce_count = (self.peers.len() as f64 * self.reduce_ratio) as usize;
        let peers_count = self.peers.len() - reduce_count;
        println!(
            "Reducing peers count from {} to {}",
            self.peers.len(),
            peers_count
        );
        for i in 0..peers_count {
            reduced_peers.push(self.peers[i].clone());
        }
        println!("Reduced peers: {:?}", reduced_peers);
        reduced_peers
    }
}
