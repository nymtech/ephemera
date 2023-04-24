use std::collections::{BTreeMap, HashMap, HashSet};

use std::thread;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc::Receiver;
use tokio::sync::oneshot::Sender;
use tokio::time::Instant;

use ephemera::configuration::Configuration;
use ephemera::crypto::PublicKey;
use ephemera::ephemera_api::EphemeraHttpClient;
use ephemera::membership::JsonPeerInfo;
use ephemera::peer::PeerId;

use crate::{PeerSettings, EPHEMERA_IP, HTTP_API_PORT_BASE};

#[async_trait]
pub(crate) trait Provider: Send + Sync + 'static {
    async fn peers(&self) -> Vec<JsonPeerInfo>;
    async fn check_broadcast_groups(&self) -> anyhow::Result<()>;
}

pub(crate) struct ProviderRunner {
    provider: Box<dyn Provider>,
    http_peers_ch: Receiver<Sender<Vec<JsonPeerInfo>>>,
}

impl ProviderRunner {
    pub(crate) fn new(
        provider: Box<dyn Provider>,
        rcv: Receiver<Sender<Vec<JsonPeerInfo>>>,
    ) -> Self {
        Self {
            provider,
            http_peers_ch: rcv,
        }
    }

    pub(crate) async fn run(mut self) -> anyhow::Result<()> {
        let wait_time = Instant::now() + std::time::Duration::from_secs(60);
        let mut interval = tokio::time::interval_at(wait_time, std::time::Duration::from_secs(15));
        loop {
            tokio::select! {
                Some(reply_tx) = self.http_peers_ch.recv() => {
                    let peers = self.provider.peers().await;
                    reply_tx.send(peers).unwrap();
                }
                _ = interval.tick() => {
                    self.provider.check_broadcast_groups().await?;
                }
            }
        }
    }
}

pub(crate) struct PeersStatus {
    all_peers: HashMap<PeerId, JsonPeerInfo>,
    clients: HashMap<PeerId, EphemeraHttpClient>,
    node_names: BTreeMap<String, PeerId>,
}

impl PeersStatus {
    pub(crate) fn new() -> Self {
        let peers = Self::read_peers_config();
        let mut clients = HashMap::new();
        let mut all_peers = HashMap::new();
        let mut nodes = BTreeMap::new();

        for (i, info) in peers.iter().enumerate() {
            let pub_key = info.public_key.parse::<PublicKey>().unwrap();
            let peer_id = PeerId::from_public_key(&pub_key);

            let url = format!("http://{EPHEMERA_IP}:{}", HTTP_API_PORT_BASE + i as u16);
            let client = EphemeraHttpClient::new(url);

            clients.insert(peer_id, client);
            all_peers.insert(peer_id, info.clone());
            nodes.insert(info.name.clone(), peer_id);
        }
        Self {
            all_peers,
            clients,
            node_names: nodes,
        }
    }
    fn read_peers_config() -> Vec<JsonPeerInfo> {
        let mut peers = vec![];
        let path = Configuration::ephemera_root_dir()
            .unwrap()
            .join("peers.toml");

        let config = std::fs::read_to_string(path).unwrap();

        let mut settings = toml::from_str::<PeerSettings>(&config).unwrap();
        settings.peers.sort_by(|a, b| a.name.cmp(&b.name));

        settings.peers.into_iter().for_each(|setting| {
            peers.push(JsonPeerInfo {
                name: setting.name,
                address: setting.address,
                public_key: setting.public_key,
            });
        });
        println!("Read {:?} peers from config", peers.len());
        peers
    }
}

/// Returns all peers.
pub(crate) struct PeersProvider {
    peers_status: PeersStatus,
}

impl PeersProvider {
    pub(crate) fn new() -> Self {
        let peers_status = PeersStatus::new();
        Self { peers_status }
    }
}

#[async_trait]
impl Provider for PeersProvider {
    async fn peers(&self) -> Vec<JsonPeerInfo> {
        self.peers_status.all_peers.values().cloned().collect()
    }

    async fn check_broadcast_groups(&self) -> anyhow::Result<()> {
        let all_peer_ids: HashSet<_> = self.peers_status.all_peers.keys().cloned().collect();

        for (peer_id, client) in &self.peers_status.clients {
            let info = client.broadcast_info().await?;
            assert_eq!(info.current_members, all_peer_ids);
            assert!(info.current_members.contains(peer_id));
        }
        println!("All peers are in broadcast groups");
        Ok(())
    }
}

/// Returns only a fraction of peers based on the reduce_ratio.
///
/// Broadcasts info should contain only peers it returns.
pub(crate) struct ReducingPeerProvider {
    peers_status: PeersStatus,
    //For example if peers length is 6 and ratio is 0.2, then 1 peer will be removed.
    reduce_ratio: f64,
}

impl ReducingPeerProvider {
    pub(crate) fn new(reduce_ratio: f64) -> Self {
        let peers_status = PeersStatus::new();
        Self {
            peers_status,
            reduce_ratio,
        }
    }

    fn get_reduced_peers(&self) -> Vec<JsonPeerInfo> {
        let mut active_peers = vec![];
        let reduce_count = (self.peers_status.all_peers.len() as f64 * self.reduce_ratio) as usize;
        let peers_count = self.peers_status.all_peers.len() - reduce_count;
        println!(
            "Reducing peers count from {} to {}",
            self.peers_status.all_peers.len(),
            peers_count
        );

        self.peers_status
            .node_names
            .values()
            .take(peers_count)
            .for_each(|peer_id| {
                active_peers.push(self.peers_status.all_peers.get(peer_id).unwrap().clone());
            });

        println!("Reduced peers: {:?}", active_peers);
        active_peers
    }
}

#[async_trait]
impl Provider for ReducingPeerProvider {
    async fn peers(&self) -> Vec<JsonPeerInfo> {
        let mut reduced_peers = vec![];
        let reduce_count = (self.peers_status.all_peers.len() as f64 * self.reduce_ratio) as usize;
        let peers_count = self.peers_status.all_peers.len() - reduce_count;
        println!(
            "Reducing peers count from {} to {}",
            self.peers_status.all_peers.len(),
            peers_count
        );

        self.peers_status
            .node_names
            .values()
            .take(peers_count)
            .for_each(|peer_id| {
                reduced_peers.push(self.peers_status.all_peers.get(peer_id).unwrap().clone());
            });

        println!("Reduced peers: {:?}", reduced_peers);
        reduced_peers
    }

    async fn check_broadcast_groups(&self) -> anyhow::Result<()> {
        let active_peers = self.get_reduced_peers();
        let active_peer_ids = active_peers
            .iter()
            .map(|peer| self.peers_status.node_names.get(&peer.name).unwrap())
            .collect::<HashSet<_>>();

        for (id, client) in &self.peers_status.clients {
            let info = client.broadcast_info().await?;
            assert!(info
                .current_members
                .iter()
                .all(|peer| active_peer_ids.contains(peer)));
            println!("{}: {:?}", id, info);
        }
        Ok(())
    }
}

/// Broadcast groups should contain all peers which are alive.
pub(crate) struct HealthCheckPeersProvider {
    peers_status: PeersStatus,
}

impl HealthCheckPeersProvider {
    pub(crate) fn new() -> Self {
        let peers_status = PeersStatus::new();
        Self { peers_status }
    }

    async fn responsive_peers(&self) -> anyhow::Result<HashSet<PeerId>> {
        let mut responsive_peers = HashSet::new();
        for (id, client) in &self.peers_status.clients {
            if client.health().await.is_ok() {
                responsive_peers.insert(*id);
            }
        }
        Ok(responsive_peers)
    }
}

#[async_trait]
impl Provider for HealthCheckPeersProvider {
    async fn peers(&self) -> Vec<JsonPeerInfo> {
        let responsive_peers = self.responsive_peers().await.unwrap();
        self.peers_status
            .all_peers
            .iter()
            .filter(|(id, _)| responsive_peers.contains(id))
            .map(|(_, info)| info.clone())
            .collect()
    }

    //TODO: check the time between this call and http peers call, so that check peers based on last peers call
    async fn check_broadcast_groups(&self) -> anyhow::Result<()> {
        let responsive_peers = self.responsive_peers().await?;
        println!("Responsive peers: {:?}", responsive_peers);

        thread::sleep(Duration::from_secs(30));

        for (id, client) in &self.peers_status.clients {
            let info = client.broadcast_info().await?;
            assert_eq!(info.current_members, responsive_peers);
            println!("{}: {:?}", id, info);
        }
        Ok(())
    }
}
