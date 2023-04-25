use std::collections::{BTreeMap, HashMap, HashSet};
use std::thread;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc::Receiver;
use tokio::sync::oneshot::Sender;

use ephemera::configuration::Configuration;
use ephemera::crypto::PublicKey;
use ephemera::ephemera_api::Client;
use ephemera::membership::JsonPeerInfo;
use ephemera::peer::PeerId;

use crate::{PeerSettings, EPHEMERA_IP, HTTP_API_PORT_BASE};

/// Returns peers from all peers based on some criteria.
///
/// For example, peers which respond to health check
#[async_trait]
pub(crate) trait Provider: Send + Sync + 'static {
    async fn peers(&self, status: &PeersStatus) -> HashSet<PeerId>;
}

/// Request peers from Provider and checks membership
pub(crate) struct ProviderRunner {
    /// All peers
    peers_status: PeersStatus,
    /// Chooses peers from all peers
    provider: Box<dyn Provider>,
    /// Channel to send peers to http server when it requests them
    http_peers_ch: Receiver<Sender<Vec<JsonPeerInfo>>>,
}

impl ProviderRunner {
    pub(crate) fn new(
        provider: Box<dyn Provider>,
        rcv: Receiver<Sender<Vec<JsonPeerInfo>>>,
    ) -> Self {
        let peers_status = PeersStatus::new();
        Self {
            peers_status,
            provider,
            http_peers_ch: rcv,
        }
    }

    ///Query peers from Provider and run check about membership
    pub(crate) async fn run(mut self) -> anyhow::Result<()> {
        println!("Starting provider runner, all peers:");
        for (id, peer) in &self.peers_status.all_peers {
            println!("Name {:?}, id: {id}\n", peer.name)
        }
        loop {
            let peers = self.provider.peers(&self.peers_status).await;
            println!(
                "Current peers: {:?}\n",
                peers.iter().map(|p| p.to_string()).collect::<Vec<String>>()
            );

            let json = self.peers_status.json_info(&peers);

            let mut peers_query_count = 0;
            loop {
                if let Some(reply_tx) = self.http_peers_ch.recv().await {
                    reply_tx.send(json.clone()).unwrap();
                    peers_query_count += 1;
                }
                //In general assume that all peers query peers list around the same time
                //So one loop captures single round of all running nodes' queries
                if peers_query_count == peers.len() {
                    break;
                }
            }

            println!("Waiting 10 sec...");
            thread::sleep(Duration::from_secs(10));
            match self.check_broadcast_groups().await {
                Ok(_) => {}
                Err(err) => {
                    println!("Error: {:?}", err);
                }
            }
        }
    }

    ///Checks if expected group matches the group returned by Ephemera nodes over http API
    async fn check_broadcast_groups(&self) -> anyhow::Result<()> {
        let peer_ids = self.provider.peers(&self.peers_status).await;
        let clients = self
            .peers_status
            .clients
            .iter()
            .filter(|(id, _)| peer_ids.contains(id))
            .collect::<HashMap<&PeerId, &Client>>();

        let mut correct_count = 0;
        for (peer_id, client) in clients.iter() {
            let info = client.broadcast_info().await?;
            if info.current_members != peer_ids {
                println!("Peer {} has wrong broadcast group: {:?}\n", peer_id, info);
            } else {
                correct_count += 1;
                println!("Peer {} has correct broadcast group: {:?}\n", peer_id, info);
            }
        }
        if correct_count == clients.len() {
            println!("All peers have correct broadcast groups\n");
        } else {
            println!("Some peers have wrong broadcast groups\n");
        }
        Ok(())
    }
}

pub(crate) struct PeersStatus {
    all_peers: HashMap<PeerId, JsonPeerInfo>,
    clients: HashMap<PeerId, Client>,
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
            let client = Client::new(url);

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

    fn json_info(&self, ids: &HashSet<PeerId>) -> Vec<JsonPeerInfo> {
        let mut json_info = vec![];
        for peer_id in ids {
            if let Some(info) = self.all_peers.get(peer_id) {
                json_info.push(info.clone());
            }
        }
        json_info
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
pub(crate) struct PeersProvider;

#[async_trait]
impl Provider for PeersProvider {
    async fn peers(&self, status: &PeersStatus) -> HashSet<PeerId> {
        status.all_peers.keys().cloned().collect()
    }
}

/// Returns only a fraction of peers based on the reduce_ratio.
///
/// Broadcasts info should contain only peers it returns.
pub(crate) struct ReducingPeerProvider {
    //For example if peers length is 6 and ratio is 0.2, then 1 peer will be removed.
    reduce_ratio: f64,
}

impl ReducingPeerProvider {
    pub(crate) fn new(reduce_ratio: f64) -> Self {
        Self { reduce_ratio }
    }
}

#[async_trait]
impl Provider for ReducingPeerProvider {
    async fn peers(&self, status: &PeersStatus) -> HashSet<PeerId> {
        let all_peers = &status.all_peers;
        let reduce_count = (all_peers.len() as f64 * self.reduce_ratio) as usize;
        let peers_count = all_peers.len() - reduce_count;
        println!(
            "Reducing peers count from {} to {}",
            all_peers.len(),
            peers_count
        );

        let mut sorted_by_name: Vec<PeerId> = all_peers.keys().cloned().collect();
        sorted_by_name.sort_by(|a, b| {
            let p1 = &all_peers.get(a).unwrap().name;
            let p2 = &all_peers.get(b).unwrap().name;
            p1.cmp(p2)
        });

        sorted_by_name.iter().take(peers_count).cloned().collect()
    }
}

/// Broadcast groups should contain all peers which are alive.
pub(crate) struct HealthCheckPeersProvider;

impl HealthCheckPeersProvider {
    async fn responsive_peers(&self, status: &PeersStatus) -> anyhow::Result<HashSet<PeerId>> {
        let mut responsive_peers = HashSet::new();
        for (id, client) in &status.clients {
            if client.health().await.is_ok() {
                responsive_peers.insert(*id);
            }
        }
        Ok(responsive_peers)
    }
}

#[async_trait]
impl Provider for HealthCheckPeersProvider {
    async fn peers(&self, status: &PeersStatus) -> HashSet<PeerId> {
        let responsive_peers = self.responsive_peers(status).await.unwrap();
        println!("Responsive peers: {:?}", responsive_peers);
        responsive_peers.into_iter().collect()
    }
}
