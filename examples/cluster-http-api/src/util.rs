use ephemera::crypto::Keypair;
use ephemera::ephemera_api;
use ephemera::ephemera_api::{
    ApiDhtQueryRequest, ApiDhtQueryResponse, ApiDhtStoreRequest, ApiEphemeraMessage, ApiHealth,
    Client, HttpClientResult, RawApiEphemeraMessage,
};

pub(crate) fn create_ephemera_message(
    label: String,
    data: Vec<u8>,
    key_pair: &Keypair,
) -> ApiEphemeraMessage {
    let message = RawApiEphemeraMessage::new(label, data);
    message.sign(key_pair).expect("Failed to sign message")
}

//Client which wraps Ephemera Client and checks health before each request
pub(crate) struct HealthyClient {
    client: Client,
}

impl HealthyClient {
    pub(crate) fn new(url: String) -> Self {
        let client = Client::new(url);
        Self { client }
    }

    pub(crate) fn new_with_timeout(url: String, timeout: u64) -> Self {
        let client = Client::new_with_timeout(url, timeout);
        Self { client }
    }

    pub(crate) async fn health(&self) -> HttpClientResult<ApiHealth> {
        let health = self.client.health().await?;
        Ok(health)
    }

    pub(crate) async fn get_ephemera_config(
        &self,
    ) -> HttpClientResult<ephemera_api::ApiEphemeraConfig> {
        self.health().await?;
        self.client.get_ephemera_config().await
    }

    pub(crate) async fn get_block_by_hash(
        &self,
        hash: &str,
    ) -> HttpClientResult<Option<ephemera_api::ApiBlock>> {
        self.health().await?;
        self.client.get_block_by_hash(hash).await
    }

    pub(crate) async fn get_block_certificates(
        &self,
        hash: &str,
    ) -> HttpClientResult<Option<Vec<ephemera_api::ApiCertificate>>> {
        self.health().await?;
        self.client.get_block_certificates(hash).await
    }

    pub(crate) async fn get_block_by_height(
        &self,
        height: u64,
    ) -> HttpClientResult<Option<ephemera_api::ApiBlock>> {
        self.health().await?;
        self.client.get_block_by_height(height).await
    }

    pub(crate) async fn get_last_block(&self) -> HttpClientResult<ephemera_api::ApiBlock> {
        self.health().await?;
        self.client.get_last_block().await
    }

    pub(crate) async fn broadcast_info(&self) -> HttpClientResult<ephemera_api::ApiBroadcastInfo> {
        self.health().await?;
        self.client.broadcast_info().await
    }

    pub(crate) async fn submit_message(&self, message: ApiEphemeraMessage) -> HttpClientResult<()> {
        self.health().await?;
        self.client.submit_message(message).await
    }

    pub(crate) async fn store_in_dht(&self, request: ApiDhtStoreRequest) -> HttpClientResult<()> {
        self.health().await?;
        self.client.store_in_dht(request).await
    }

    pub(crate) async fn query_dht(
        &self,
        request: ApiDhtQueryRequest,
    ) -> HttpClientResult<Option<ApiDhtQueryResponse>> {
        self.health().await?;
        self.client.query_dht(request).await
    }
}
