use std::collections::HashMap;

use anyhow::anyhow;

use ephemera::configuration::Configuration;
use ephemera::crypto::{EphemeraKeypair, EphemeraPublicKey, Keypair, PublicKey};

pub(crate) type PeerId = String;

#[derive(Debug, Clone)]
pub struct NymPeer {
    pub name: String,
    pub address: String,
    pub public_key: PublicKey,
    pub peer_id: PeerId,
}

impl NymPeer {
    pub(crate) fn new(
        name: String,
        address: String,
        public_key: PublicKey,
        peer_id: PeerId,
    ) -> Self {
        Self {
            name,
            address,
            public_key,
            peer_id,
        }
    }
}

// Information about other Nym-Apis.
pub(crate) struct NymApiEphemeraPeerInfo {
    pub(crate) local_peer: NymPeer,
    pub(crate) peers: HashMap<PeerId, NymPeer>,
}

impl NymApiEphemeraPeerInfo {
    fn new(local_peer: NymPeer, peers: HashMap<PeerId, NymPeer>) -> Self {
        Self { local_peer, peers }
    }

    pub(crate) fn get_peers_count(&self) -> usize {
        self.peers.len()
    }

    //LOCAL DEV CLUSTER ONLY
    //Get peers from dev Ephemera cluster config files
    pub(crate) fn from_ephemera_dev_cluster_conf(
        conf: &Configuration,
    ) -> anyhow::Result<NymApiEphemeraPeerInfo> {
        let node_info = conf.node.clone();

        let keypair = bs58::decode(&node_info.private_key).into_vec().unwrap();
        let keypair = Keypair::from_raw_vec(keypair).unwrap();
        let local_peer_id = keypair.public_key().to_base58();

        let home_path = dirs::home_dir()
            .ok_or(anyhow!("Failed to get home dir"))?
            .join(".ephemera")
            .join("nym-api");
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

                let conf = Configuration::try_load_application("nym-api", node_name)
                    .unwrap_or_else(|_| panic!("Error loading configuration for node {node_name}"));

                let node_info = conf.node;

                let keypair = bs58::decode(&node_info.private_key).into_vec().unwrap();
                let keypair = Keypair::from_raw_vec(keypair).unwrap();

                let peer_id = keypair.public_key().to_base58();

                let peer = NymPeer::new(
                    peer_id.clone(),
                    node_info.address.clone(),
                    keypair.public_key(),
                    peer_id.clone(),
                );

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
