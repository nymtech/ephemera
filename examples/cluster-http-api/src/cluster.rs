use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;

use ephemera::crypto::Keypair;

use crate::node::Node;
use crate::util::create_ephemera_message;
use crate::Args;

pub(crate) struct Cluster {
    args: Args,
    nodes: Arc<tokio::sync::Mutex<Vec<Node>>>,
    keypair: Arc<Keypair>,
}

impl Cluster {
    pub fn new(args: Args, nodes: Vec<Node>, keypair: Keypair) -> Self {
        Self {
            args,
            nodes: Arc::new(tokio::sync::Mutex::new(nodes)),
            keypair: Arc::new(keypair),
        }
    }

    /// Submit messages to different nodes at the same interval.
    /// Which means that blocks from different nodes will include almost identical set of messages.
    pub(crate) async fn submit_messages_to_all_nodes_at_the_same_interval(
        &self,
        interval: Duration,
    ) -> anyhow::Result<JoinHandle<()>> {
        let nodes = self.nodes.clone();
        let keypair = self.keypair.clone();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);

            let mut tick_counter = 0;
            loop {
                interval.tick().await;

                let label = format!("message_{}", tick_counter);
                for node in nodes.lock().await.iter_mut() {
                    let message = create_ephemera_message(label.clone(), vec![0], &keypair);
                    match node.submit_message(message.clone()).await {
                        Ok(_) => {
                            node.add_pending_message(message);
                        }
                        Err(err) => {
                            log::error!(
                                "Error while submitting message {} to node {}: {}",
                                label,
                                node.id,
                                err
                            );
                        }
                    }
                }
                log::debug!("Submitted message {} to all nodes", label);
                tick_counter += 1;
            }
        });
        Ok(handle)
    }

    ///Query blocks by height from all nodes at the same interval.
    pub(crate) async fn query_blocks_by_height(
        &mut self,
        interval: Duration,
    ) -> anyhow::Result<JoinHandle<()>> {
        let _args = self.args.clone();

        let nodes = self.nodes.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);

            loop {
                interval.tick().await;

                for node in nodes.lock().await.iter_mut() {
                    log::info!(
                        "Querying block with height {} from node {}",
                        node.last_asked_block_height,
                        node.id
                    );

                    match node
                        .get_block_and_certificates_by_height(node.last_asked_block_height)
                        .await
                    {
                        Ok(Some((block, certificates))) => {
                            log::debug!(
                                "Block with height {} found node {}",
                                node.last_asked_block_height,
                                node.id
                            );
                            node.process_block_with_next_height(block, certificates);
                        }
                        Ok(None) => {
                            log::warn!(
                                "Block with height {} not found node {}",
                                node.last_asked_block_height,
                                node.id
                            );
                        }
                        Err(err) => {
                            log::error!(
                                "Error while querying block with height {} from node {}: {}",
                                node.last_asked_block_height,
                                node.id,
                                err
                            );
                        }
                    }
                }
            }
        });
        Ok(handle)
    }

    ///Query last block from all nodes at the same interval.
    pub(crate) async fn query_last_block(
        &self,
        interval: Duration,
    ) -> anyhow::Result<JoinHandle<()>> {
        let _args = self.args.clone();

        let nodes = self.nodes.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);

            loop {
                interval.tick().await;

                for node in nodes.lock().await.iter() {
                    let block = node.get_last_block().await.unwrap();
                    log::info!("Last block from node {}: {:?}", node.id, block);

                    if node.last_block == block {
                        log::debug!("Last block from node {} is the same as the last block from the previous query", node.id);
                    } else {
                        log::debug!("Last block from node {} is different from the last block from the previous query", node.id);
                    }
                }
            }
        });
        Ok(handle)
    }

    ///Query blocks by hash
    pub(crate) async fn query_blocks_by_hash(
        &mut self,
        interval: Duration,
    ) -> anyhow::Result<JoinHandle<()>> {
        let _args = self.args.clone();

        let nodes = self.nodes.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);

            loop {
                interval.tick().await;

                for node in nodes.lock().await.iter_mut() {
                    if let Some(hash) = node.next_pending_block_hash() {
                        log::trace!("Querying block with hash {} from node {}", hash, node.id);

                        match node.get_block_by_hash(hash.as_str()).await {
                            Ok(Some(_block)) => {
                                log::debug!("Block with hash {} found node {}", hash, node.id);
                                node.remove_pending_block_hash(hash.as_str());
                            }
                            Ok(None) => {
                                log::error!("Block with hash {} not found", hash);
                            }
                            Err(err) => {
                                log::error!(
                                    "Error while querying block with hash {}: {}",
                                    hash,
                                    err
                                );
                            }
                        }
                    }
                }
            }
        });
        Ok(handle)
    }

    pub(crate) async fn store_dht(&self, interval: Duration) -> anyhow::Result<JoinHandle<()>> {
        let _args = self.args.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);
            loop {
                interval.tick().await;
            }
        });
        Ok(handle)
    }

    pub(crate) async fn query_dht(&self, interval: Duration) -> anyhow::Result<JoinHandle<()>> {
        let _args = self.args.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);
            loop {
                interval.tick().await;
            }
        });
        Ok(handle)
    }
}
