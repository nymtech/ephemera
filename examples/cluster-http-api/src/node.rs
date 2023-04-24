use ephemera::ephemera_api;
use log::{info, warn};
use std::sync::atomic::{AtomicU64, Ordering};

use ephemera::ephemera_api::{
    ApiBlock, ApiCertificate, ApiEphemeraConfig, ApiEphemeraMessage, ApiHealth, EphemeraHttpClient,
};

pub(crate) struct Node {
    pub(crate) id: usize,
    pub(crate) url: String,
    pub(crate) ephemera_config: ApiEphemeraConfig,
    pub(crate) last_block_height: AtomicU64,
    pub(crate) last_asked_block_height: AtomicU64,
    pub(crate) last_block: ApiBlock,
    pending_messages: Vec<ApiEphemeraMessage>,
    pending_block_hashes: Vec<String>,
}

impl Node {
    pub(crate) async fn init(id: usize, url: String) -> Self {
        //Use separate client instances elsewhere so they don't block each other
        let client = EphemeraHttpClient::new(url.clone());
        let last_block = client.get_last_block().await.unwrap();
        let ephemera_config = client.get_ephemera_config().await.unwrap();
        info!("Node {} config {:?}", id, ephemera_config);
        Self {
            id,
            url,
            pending_messages: vec![],
            pending_block_hashes: vec![],
            last_block_height: AtomicU64::new(last_block.header.height),
            last_asked_block_height: AtomicU64::new(last_block.header.height),
            last_block,
            ephemera_config,
        }
    }

    pub(crate) fn add_pending_message(&mut self, message: ApiEphemeraMessage) {
        self.pending_messages.push(message);
    }

    pub(crate) fn remove_pending_message(&mut self, message: &ApiEphemeraMessage) {
        self.pending_messages.retain(|m| m != message);
    }

    pub(crate) fn add_pending_block_hash(&mut self, hash: String) {
        self.pending_block_hashes.push(hash);
    }

    pub(crate) fn remove_pending_block_hash(&mut self, hash: &str) {
        self.pending_block_hashes.retain(|h| h != hash);
    }

    pub(crate) fn pending_block_hashes(&self) -> Vec<String> {
        self.pending_block_hashes.clone()
    }

    pub(crate) fn process_block_with_next_height(
        &mut self,
        block: ApiBlock,
        certificates: Vec<ApiCertificate>,
    ) {
        let hash = block.header.hash.clone();
        let height = block.header.height;
        let messages = block.messages.clone();

        info!(
            "Block {} height {}, node {}, signatures count {}, messages count {}",
            hash,
            height,
            self.id,
            certificates.len(),
            block.messages.len()
        );
        self.last_block = block;
        //TODO: check gaps, "force" gaps
        self.last_asked_block_height.fetch_add(1, Ordering::Acquire);

        if !self.pending_messages.is_empty() {
            info!(
                "Node {} pending messages count {}",
                self.id,
                self.pending_messages.len()
            );

            for message in messages {
                self.remove_pending_message(&message);
            }

            if !self.pending_messages.is_empty() {
                warn!(
                    "Node {} pending messages count after new block {}",
                    self.id,
                    self.pending_messages.len()
                );
            } else {
                info!("Node {} pending messages removed", self.id);
            }
        }

        self.add_pending_block_hash(hash);
    }

    pub(crate) async fn get_node_config(
        &self,
        client: &mut EphemeraHttpClient,
    ) -> anyhow::Result<ephemera_api::ApiEphemeraConfig> {
        client.get_ephemera_config().await.map_err(|e| e.into())
    }

    pub(crate) async fn health(
        &self,
        client: &mut EphemeraHttpClient,
    ) -> anyhow::Result<ApiHealth> {
        client.health().await.map_err(|e| e.into())
    }

    pub(crate) async fn get_block_by_hash(
        &self,
        client: &mut EphemeraHttpClient,
        hash: &str,
    ) -> anyhow::Result<Option<ApiBlock>> {
        client.get_block_by_hash(hash).await.map_err(|e| e.into())
    }

    pub(crate) async fn get_block_certificates(
        &self,
        client: &mut EphemeraHttpClient,
        hash: &str,
    ) -> anyhow::Result<Option<Vec<ApiCertificate>>> {
        client
            .get_block_certificates(hash)
            .await
            .map_err(|e| e.into())
    }

    pub(crate) async fn get_block_and_certificates_by_height(
        &self,
        client: &mut EphemeraHttpClient,
        height: u64,
    ) -> anyhow::Result<Option<(ApiBlock, Vec<ApiCertificate>)>> {
        match client.get_block_by_height(height).await {
            Ok(Some(block)) => {
                let certificates = self
                    .get_block_certificates(client, &block.header.hash)
                    .await?
                    .unwrap();
                Ok(Some((block, certificates)))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub(crate) async fn get_last_block(
        &self,
        client: &mut EphemeraHttpClient,
    ) -> anyhow::Result<ApiBlock> {
        client.get_last_block().await.map_err(|e| e.into())
    }

    pub(crate) async fn submit_message(
        &self,
        client: &mut EphemeraHttpClient,
        message: ApiEphemeraMessage,
    ) -> anyhow::Result<()> {
        client.submit_message(message).await.map_err(|e| e.into())
    }
}
