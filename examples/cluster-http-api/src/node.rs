use ephemera::ephemera_api::{
    ApiBlock, ApiCertificate, ApiEphemeraMessage, EphemeraHttpClient, Health,
};

pub(crate) struct Node {
    pub(crate) id: u64,
    pub(crate) last_asked_block_height: u64,
    pub(crate) last_block: ApiBlock,
    client: EphemeraHttpClient,
    pending_messages: Vec<ApiEphemeraMessage>,
    pending_block_hashes: Vec<String>,
}

impl Node {
    pub(crate) async fn init(id: u64, url: String) -> Self {
        let client = EphemeraHttpClient::new(url.to_string());
        let last_block = client.get_last_block().await.unwrap();
        Self {
            id,
            client,
            pending_messages: vec![],
            pending_block_hashes: vec![],
            last_asked_block_height: last_block.header.height,
            last_block,
        }
    }

    pub(crate) fn add_pending_message(&mut self, message: ApiEphemeraMessage) {
        self.pending_messages.push(message);
    }

    pub(crate) fn remove_pending_message(&mut self, message: &ApiEphemeraMessage) {
        self.pending_messages.retain(|m| m != message);
    }

    pub(crate) fn has_pending_message(&self, message: &ApiEphemeraMessage) -> bool {
        self.pending_messages.contains(message)
    }

    pub(crate) fn add_pending_block_hash(&mut self, hash: String) {
        self.pending_block_hashes.push(hash);
    }

    pub(crate) fn remove_pending_block_hash(&mut self, hash: &str) {
        self.pending_block_hashes.retain(|h| h != hash);
    }

    pub(crate) fn next_pending_block_hash(&self) -> Option<String> {
        self.pending_block_hashes.first().cloned()
    }

    pub(crate) fn process_block_with_next_height(
        &mut self,
        block: ApiBlock,
        certificates: Vec<ApiCertificate>,
    ) {
        let hash = block.header.hash.clone();
        let height = block.header.height;
        let messages = block.messages.clone();
        log::info!(
            "Block {} height {}, node {}, signatures count {}, messages count {}",
            hash,
            height,
            self.id,
            certificates.len(),
            block.messages.len()
        );
        self.last_block = block;
        //TODO: check gaps, "force" gaps
        self.last_asked_block_height += 1;

        log::info!(
            "Node {} pending messages count {}",
            self.id,
            self.pending_messages.len()
        );
        for message in messages {
            self.remove_pending_message(&message);
        }
        log::info!(
            "Node {} pending messages count after new block {}",
            self.id,
            self.pending_messages.len()
        );
    }

    //Call health in api/http module
    pub(crate) async fn health(&self) -> anyhow::Result<Health> {
        self.client.health().await.map_err(|e| e.into())
    }

    //Call block_by_hash in api/http module
    pub(crate) async fn get_block_by_hash(&self, hash: &str) -> anyhow::Result<Option<ApiBlock>> {
        self.client
            .get_block_by_hash(hash)
            .await
            .map_err(|e| e.into())
    }

    //Call block_certificates in api/http module
    pub(crate) async fn get_block_certificates(
        &self,
        hash: &str,
    ) -> anyhow::Result<Option<Vec<ApiCertificate>>> {
        self.client
            .get_block_certificates(hash)
            .await
            .map_err(|e| e.into())
    }

    //Call block_by_height in api/http module
    pub(crate) async fn get_block_and_certificates_by_height(
        &self,
        height: u64,
    ) -> anyhow::Result<Option<(ApiBlock, Vec<ApiCertificate>)>> {
        match self.client.get_block_by_height(height).await {
            Ok(Some(block)) => {
                let certificates = self
                    .get_block_certificates(&block.header.hash)
                    .await?
                    .unwrap();
                Ok(Some((block, certificates)))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub(crate) async fn get_last_block(&self) -> anyhow::Result<ApiBlock> {
        self.client.get_last_block().await.map_err(|e| e.into())
    }

    pub(crate) async fn submit_message(&self, message: ApiEphemeraMessage) -> anyhow::Result<()> {
        self.client
            .submit_message(message)
            .await
            .map_err(|e| e.into())
    }
}
