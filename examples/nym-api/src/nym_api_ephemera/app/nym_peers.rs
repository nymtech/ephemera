use std::collections::HashMap;

use anyhow::anyhow;

use ephemera::config::Configuration;
use ephemera::utilities::{Ed25519Keypair, Keypair, PeerId, ToPeerId};

//We don't know where this information comes yet, but it should be something like this
#[derive(Clone)]
pub(crate) struct Peer {
    /// The address of a Nym-API. We may need it to query rewards.
    /// For example when another API is the first one to submit final rewards to smart contract,
    /// we need to know its API address to query those rewards.
    pub(crate) address: String,
    /// The peer id of the Nym-API.
    /// When using libp2p PeerId concept, then PeerId is cryptographically secure identifier because it's derived from
    /// the public key of the node.
    pub(crate) peer_id: PeerId,
}

impl Peer {
    pub(crate) fn new(address: String, peer_id: PeerId) -> Self {
        Self { address, peer_id }
    }
}

/// Information about other Nym-Apis.
pub(crate) struct NymApiEphemeraPeerInfo {
    /// Information about local Nym-API.
    pub(crate) local_peer: Peer,
    /// Information about other Nym-APIs.
    pub(crate) peers: HashMap<PeerId, Peer>,
}

impl NymApiEphemeraPeerInfo {
    fn new(local_peer: Peer, peers: HashMap<PeerId, Peer>) -> Self {
        Self { local_peer, peers }
    }

    pub(crate) fn get_peer_by_id(&self, peer_id: &PeerId) -> Option<&Peer> {
        self.peers.get(peer_id)
    }

    pub(crate) fn get_peers_count(&self) -> usize {
        self.peers.len()
    }

    //DEV ONLY - get peers from dev Ephemera cluster config files
    pub(crate) fn from_ephemera_dev_cluster_conf(
        conf: &Configuration,
    ) -> anyhow::Result<NymApiEphemeraPeerInfo> {
        let node_info = conf.node_config.clone();

        let keypair = bs58::decode(&node_info.private_key).into_vec().unwrap();
        let keypair = Ed25519Keypair::from_raw_vec(keypair).unwrap();
        let local_peer_id = keypair.peer_id();

        let home_path = dirs::home_dir()
            .ok_or(anyhow!("Failed to get home dir"))?
            .join(".ephemera");
        let home_dir = std::fs::read_dir(home_path)?;

        let mut peers = HashMap::new();
        for entry in home_dir {
            let path = entry?.path();
            if path.is_dir() {
                let node_name = path
                    .file_name()
                    .ok_or(anyhow!("Failed to read file"))?
                    .to_str()
                    .ok_or(anyhow!("Failed to read file"))?;

                if !node_name.starts_with("node") {
                    continue;
                }

                log::info!("Loading config for node {}", node_name);

                let conf = Configuration::try_load_from_home_dir(node_name)
                    .unwrap_or_else(|_| panic!("Error loading configuration for node {node_name}"));

                let node_info = conf.node_config;

                let keypair = bs58::decode(&node_info.private_key).into_vec().unwrap();
                let keypair = Ed25519Keypair::from_raw_vec(keypair).unwrap();

                let peer_id = keypair.peer_id();

                let peer = Peer::new(node_info.address, peer_id);

                peers.insert(peer_id, peer);

                log::info!("Loaded config for node {}", node_name);
            }
        }

        let local_peer = peers
            .get(&local_peer_id)
            .ok_or(anyhow!("Failed to get local peer"))?
            .clone();
        let peer_info = NymApiEphemeraPeerInfo::new(local_peer, peers);
        Ok(peer_info)
    }
}
