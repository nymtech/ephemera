use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use log::{debug, error, info, trace, warn};
use rand::Rng;
use tokio::task::JoinHandle;

use ephemera::crypto::{EphemeraKeypair, Keypair};
use ephemera::ephemera_api::{ApiDhtQueryRequest, ApiDhtStoreRequest, Client};

use crate::node::Node;
use crate::util::create_ephemera_message;
use crate::{node, Args};

const EPHEMERA_IP: &str = "127.0.0.1";
// Node 1 http port is 7000, Node2 http port is 7001, etc.
const HTTP_API_PORT_BASE: u16 = 7000;

pub(crate) struct Cluster {
    nodes: Arc<tokio::sync::Mutex<Vec<Node>>>,
    keypair: Arc<Keypair>,
    dht_pending_stores: Arc<tokio::sync::Mutex<Vec<ApiDhtStoreRequest>>>,
}

impl Cluster {
    pub async fn init(args: Args) -> Self {
        let keypair = Keypair::generate(None);
        let mut nodes = vec![];
        for i in 0..args.nr_of_nodes {
            let node = node::Node::init(
                i,
                format!("http://{}:{}", EPHEMERA_IP, HTTP_API_PORT_BASE as usize + i),
            )
            .await;
            nodes.push(node);
        }

        for node in nodes.iter_mut() {
            info!("Node {} last block: {}", node.id, node.last_block);
        }
        Self {
            nodes: Arc::new(tokio::sync::Mutex::new(nodes)),
            keypair: Arc::new(keypair),
            dht_pending_stores: Arc::new(tokio::sync::Mutex::new(vec![])),
        }
    }

    /// Submit messages to different nodes at the same interval.
    /// Which means that blocks from different nodes will include almost identical set of messages.
    #[allow(dead_code)]
    pub(crate) async fn submit_messages_to_all_nodes_at_the_same_interval(
        &self,
        interval_sec: u64,
    ) -> anyhow::Result<JoinHandle<()>> {
        let keypair = self.keypair.clone();
        let nodes = self.nodes.clone();

        if nodes.lock().await.is_empty() {
            anyhow::bail!("No nodes to submit messages to");
        }

        let mut clients = self.clients().await;
        let mut interval = tokio::time::interval(Duration::from_millis(interval_sec));
        let mut tick_counter = 0;

        loop {
            interval.tick().await;

            let label = format!("message_{}", tick_counter);
            for node in nodes.lock().await.iter_mut() {
                let message = create_ephemera_message(label.clone(), vec![0], &keypair);
                let client = clients.get_mut(&node.id).unwrap();
                match node.submit_message(client, message.clone()).await {
                    Ok(_) => {
                        node.add_pending_message(message);
                    }
                    Err(err) => {
                        error!(
                            "Error while submitting message {} to node {}: {}",
                            label, node.id, err
                        );
                    }
                }
            }
            debug!("Submitted message {} to all nodes", label);
            tick_counter += 1;
        }
    }

    //Gossip should broadcast messages to all nodes
    #[allow(dead_code)]
    pub(crate) async fn submit_messages_to_an_random_node_and_make_a_break(
        &self,
        interval_sec: u64,
    ) -> anyhow::Result<JoinHandle<()>> {
        let keypair = self.keypair.clone();
        let nodes = self.nodes.clone();
        let nodes_len = nodes.lock().await.len();

        if nodes.lock().await.is_empty() {
            anyhow::bail!("No nodes to submit messages to");
        }

        let mut clients = self.clients().await;
        let mut interval = tokio::time::interval(Duration::from_millis(interval_sec));
        let mut tick_counter = 0;

        loop {
            interval.tick().await;

            let label = format!("message_{}", tick_counter);
            let index = rand::thread_rng().gen_range(0..nodes_len);

            if let Some(node) = nodes.lock().await.get_mut(index) {
                let client = clients.get_mut(&index).unwrap();

                let message = create_ephemera_message(label.clone(), vec![0], &keypair);

                match node.submit_message(client, message.clone()).await {
                    Ok(_) => {
                        //TODO: because of gossip, some messages are expected to stay in pending
                        //But they should be in some blocks
                        node.add_pending_message(message);
                    }
                    Err(err) => {
                        error!(
                            "Error while submitting message {} to node {}: {}",
                            label, node.id, err
                        );
                    }
                }
                debug!("Submitted message {} to {}", label, node.id);
                tick_counter += 1;
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) async fn submit_messages_to_at_random_burst_and_wait(&self, nr_of_messages: usize) {
        let keypair = self.keypair.clone();
        let nodes = self.nodes.clone();
        let nodes_len = nodes.lock().await.len();

        if nodes.lock().await.is_empty() {
            error!("No nodes to submit messages to");
            return;
        }

        let avg_interval = self.avg_block_interval().await;
        let mut clients = self.clients().await;

        let mut interval = tokio::time::interval(avg_interval);
        let mut tick_counter = 0;

        loop {
            interval.tick().await;

            for _ in 0..nr_of_messages {
                let label = format!("message_{}", tick_counter);
                let index = rand::thread_rng().gen_range(0..nodes_len);

                if let Some(node) = nodes.lock().await.get_mut(index) {
                    let client = clients.get_mut(&index).unwrap();

                    let message = create_ephemera_message(label.clone(), vec![0], &keypair);

                    match node.submit_message(client, message.clone()).await {
                        Ok(_) => {
                            //TODO: because of gossip, some messages are expected to stay in pending
                            //But they should be in some blocks
                            node.add_pending_message(message);
                        }
                        Err(err) => {
                            error!(
                                "Error while submitting message {} to node {}: {}",
                                label, node.id, err
                            );
                        }
                    }
                    debug!("Submitted message {} to {}", label, node.id);
                    tick_counter += 1;
                }
            }
        }
    }

    ///Query blocks by height from all nodes at the same interval.
    pub(crate) async fn query_blocks_by_height(&self) {
        if self.nodes.lock().await.is_empty() {
            error!("No nodes to submit messages to");
            return;
        }

        let avg_interval = self.avg_block_interval().await;
        let nodes = self.nodes.clone();
        let mut interval = tokio::time::interval(avg_interval);
        let mut clients = self.clients().await;

        loop {
            interval.tick().await;

            for node in nodes.lock().await.iter_mut() {
                let last_asked_block_height = node.last_asked_block_height.load(Ordering::Relaxed);
                info!(
                    "Querying block with height {} from node {}",
                    last_asked_block_height, node.id
                );

                let mut last_height = last_asked_block_height;
                loop {
                    let client = clients.get_mut(&node.id).unwrap();

                    match node
                        .get_block_and_certificates_by_height(client, last_height)
                        .await
                    {
                        Ok(Some((block, certificates))) => {
                            debug!("Block with height {} found node {}", last_height, node.id);
                            node.process_block_with_next_height(block, certificates);
                            if last_height < last_asked_block_height {
                                last_height += 1;
                            } else {
                                break;
                            }
                        }
                        Ok(None) => {
                            warn!(
                                "Block with height {} not found node {}",
                                last_height, node.id
                            );
                            break;
                        }
                        Err(err) => {
                            error!(
                                "Error while querying block with height {} from node {}: {}",
                                last_height, node.id, err
                            );
                            break;
                        }
                    }
                }
            }
        }
    }

    ///Query last block from all nodes at the same interval.
    pub(crate) async fn query_last_block(&self) -> anyhow::Result<JoinHandle<()>> {
        if self.nodes.lock().await.is_empty() {
            anyhow::bail!("No nodes to submit messages to");
        }

        let avg_interval = self.avg_block_interval().await;
        let nodes = self.nodes.clone();
        let mut interval = tokio::time::interval(avg_interval);
        let mut clients = self.clients().await;

        loop {
            interval.tick().await;

            for node in nodes.lock().await.iter_mut() {
                let client = clients.get_mut(&node.id).unwrap();
                let block = node.get_last_block(client).await.unwrap();
                node.last_block_height
                    .store(block.header.height, Ordering::Release);

                debug!("Last block from node {}: {:?}", node.id, block);

                if node.last_block == block {
                    trace!("Last block from node {} is the same as the last block from the previous query", node.id);
                } else {
                    trace!("Last block from node {} is different from the last block from the previous query", node.id);
                }
            }
        }
    }

    ///Query blocks by hash
    pub(crate) async fn query_blocks_by_hash(&self) {
        if self.nodes.lock().await.is_empty() {
            error!("No nodes to submit messages to");
        }

        let nodes = self.nodes.clone();
        let avg_interval = self.avg_block_interval().await.checked_mul(4).unwrap();
        let mut interval = tokio::time::interval(avg_interval);
        let mut clients = self.clients().await;

        loop {
            interval.tick().await;

            for node in nodes.lock().await.iter_mut() {
                let client = clients.get_mut(&node.id).unwrap();
                let hashes = node.pending_block_hashes();

                if !hashes.is_empty() {
                    info!(
                        "Querying blocks with nr of hashes {:?} from node {}",
                        hashes.len(),
                        node.id
                    );

                    for hash in hashes {
                        match node.get_block_by_hash(client, hash.as_str()).await {
                            Ok(Some(_block)) => {
                                debug!("Block with hash {} found node {}", hash, node.id);
                                node.remove_pending_block_hash(hash.as_str());
                            }
                            Ok(None) => {
                                error!("Block with hash {} not found for node {}", hash, node.id);
                            }
                            Err(err) => {
                                error!("Error while querying block with hash {}: {}", hash, err);
                            }
                        }
                    }
                    if !node.pending_block_hashes().is_empty() {
                        warn!("Node {} still has pending block hashes", node.id);
                    } else {
                        info!("Node {} has no pending block hashes left", node.id);
                    }
                }
            }
        }
    }

    pub(crate) async fn store_in_dht_using_random_node(&self, interval: Duration) {
        let nodes = self.nodes.clone();
        let nodes_len = nodes.lock().await.len();
        let mut clients = self.clients().await;
        let dht_pending_stores = self.dht_pending_stores.clone();

        let mut tick_counter = 0;
        let mut interval = tokio::time::interval(interval);

        loop {
            interval.tick().await;
            let index = rand::thread_rng().gen_range(0..nodes_len);

            if let Some(node) = nodes.lock().await.get_mut(index) {
                let client = clients.get_mut(&index).unwrap();

                let key = format!("message_{}", tick_counter);
                let value = format!("message_{}_{}", tick_counter, node.id);
                info!("Stored key value pair in DHT: {:?} {:?}", key, value);

                let key = key.as_bytes();
                let value = value.as_bytes();
                let request = ApiDhtStoreRequest::new(key, value);

                client.store_in_dht(request.clone()).await.unwrap();

                dht_pending_stores.lock().await.push(request);

                tick_counter += 1;
            }
        }
    }

    pub(crate) async fn query_dht_using_random_node(&self, interval: Duration) {
        let nodes = self.nodes.clone();
        let nodes_len = nodes.lock().await.len();
        let mut clients = self.clients().await;

        let dht_pending_stores = self.dht_pending_stores.clone();

        let mut interval = tokio::time::interval(interval);

        loop {
            interval.tick().await;

            info!(
                "DHT pending stores before queries: {:?}",
                dht_pending_stores.lock().await.len()
            );

            let mut found = vec![];
            for store in dht_pending_stores.lock().await.iter() {
                info!(
                    "DHT pending store: {:?}",
                    String::from_utf8_lossy(store.key().as_slice())
                );

                let index = rand::thread_rng().gen_range(0..nodes_len);

                let client = clients.get_mut(&index).unwrap();

                let request = ApiDhtQueryRequest::new(store.key().as_slice());

                let response = client.query_dht(request).await.unwrap();
                match response {
                    Some(response) => {
                        info!(
                            "DHT query response: key: {:?}, value: {:?}",
                            String::from_utf8_lossy(response.key().as_slice()),
                            String::from_utf8_lossy(response.value().as_slice())
                        );
                        found.push(store.key().to_vec());
                    }
                    None => {
                        error!("DHT query response is None");
                    }
                }
            }

            let mut guard = dht_pending_stores.lock().await;
            for key in found.into_iter() {
                guard.retain(|store| store.key() != key);
            }

            info!("DHT pending stores after queries: {:?}", guard.len());
        }
    }

    async fn avg_block_interval(&self) -> Duration {
        let mut avg_interval = 0;
        for node in self.nodes.lock().await.iter() {
            avg_interval += node.ephemera_config.block_creation_interval_sec;
        }
        avg_interval /= self.nodes.lock().await.len() as u64;
        info!("Average block creation interval: {} sec", avg_interval);
        Duration::from_secs(avg_interval)
    }

    async fn clients(&self) -> HashMap<usize, Client> {
        let mut clients = HashMap::new();
        for node in self.nodes.lock().await.iter() {
            let client = Client::new_with_timeout(node.url.clone(), 30);
            clients.insert(node.id, client);
        }
        clients
    }
}
