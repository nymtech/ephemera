use std::time::Duration;

use thiserror::Error;

use crate::api::types::Health;
use crate::ephemera_api::{ApiBlock, ApiCertificate, ApiDhtQueryRequest, ApiDhtQueryResponse, ApiDhtStoreRequest, ApiEphemeraConfig, ApiEphemeraMessage};

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Internal(#[from] reqwest::Error),
    #[error("Unexpected response: {status} {body}")]
    UnexpectedResponse {
        status: reqwest::StatusCode,
        body: String,
    },
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

/// A client for the Ephemera http api.
pub struct EphemeraHttpClient {
    pub(crate) client: reqwest::Client,
    pub(crate) url: String,
}

impl EphemeraHttpClient {
    /// Create a new client.
    ///
    /// # Parameters
    /// * `url` - The url of the node api endpoint.
    pub fn new(url: String) -> Self {
        let client = reqwest::Client::new();
        Self { client, url }
    }

    /// Create a new client.
    ///
    /// # Parameters
    /// * `url` - The url of the node api endpoint.
    /// * `timeout_sec` - Request timeout in seconds.
    pub fn new_with_timeout(url: String, timeout_sec: u64) -> Self {
        let client = reqwest::ClientBuilder::new()
            .timeout(Duration::from_secs(timeout_sec))
            .build()
            .unwrap();
        Self { client, url }
    }

    /// Get the health of the node.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ephemera::ephemera_api::{EphemeraHttpClient, Health};
    ///
    /// #[tokio::main]
    ///async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///   let client = EphemeraHttpClient::new("http://localhost:7000".to_string());
    ///   let health = client.health().await.unwrap();
    ///    Ok(())
    /// }
    /// ```
    pub async fn health(&self) -> Result<Health> {
        self.query("ephemera/node/health").await
    }

    /// Get the block by hash.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ephemera::ephemera_api::{ApiBlock, EphemeraHttpClient};
    ///
    /// #[tokio::main]
    ///async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = EphemeraHttpClient::new("http://localhost:7000".to_string());
    ///     let block = client.get_block_by_hash("hash").await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_block_by_hash(&self, hash: &str) -> Result<Option<ApiBlock>> {
        let url = format!("ephemera/broadcast/block/{hash}", );
        self.query_optional(&url).await
    }

    /// Get the block certificates by hash.
    ///
    /// # Example
    /// ```no_run
    /// use ephemera::ephemera_api::{ApiCertificate, EphemeraHttpClient};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///    let client = EphemeraHttpClient::new("http://localhost:7000".to_string());
    ///    let certificates = client.get_block_certificates("hash").await?;
    ///    Ok(())
    /// }
    /// ```
    pub async fn get_block_certificates(&self, hash: &str) -> Result<Option<Vec<ApiCertificate>>> {
        let url = format!("ephemera/broadcast/block/certificates/{hash}", );
        self.query_optional(&url).await
    }

    /// Get the block by height.
    ///
    /// # Example
    /// ```no_run
    /// use ephemera::ephemera_api::{ApiBlock, EphemeraHttpClient};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///   let client = EphemeraHttpClient::new("http://localhost:7000/".to_string());
    ///   let block = client.get_block_by_height(1).await?;
    ///   Ok(())
    /// }
    pub async fn get_block_by_height(&self, height: u64) -> Result<Option<ApiBlock>> {
        let url = format!("ephemera/broadcast/block/height/{height}", );
        self.query_optional(&url).await
    }

    /// Get the last block.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ephemera::ephemera_api::{ApiBlock, EphemeraHttpClient};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///    let client = EphemeraHttpClient::new("http://localhost:7000/".to_string());
    ///    let block = client.get_last_block().await?;
    ///    Ok(())
    /// }
    pub async fn get_last_block(&self) -> Result<ApiBlock> {
        self.query("ephemera/broadcast/blocks/last").await
    }

    /// Get the node configuration.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ephemera::ephemera_api::{ApiEphemeraConfig, EphemeraHttpClient};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///    let client = EphemeraHttpClient::new("http://localhost:7000/".to_string());
    ///    let config = client.get_ephemera_config().await?;
    ///    Ok(())
    /// }
    pub async fn get_ephemera_config(&self) -> Result<ApiEphemeraConfig> {
        self.query("ephemera/node/config").await
    }

    /// Submit a message to the node.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ephemera::ephemera_api::{ApiEphemeraMessage, EphemeraHttpClient};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///   let client = EphemeraHttpClient::new("http://localhost:7000/".to_string());
    ///   let message = unimplemented!("See how to create a ApiEphemeraMessage");
    ///   client.submit_message(message).await?;
    ///   Ok(())
    /// }
    pub async fn submit_message(&self, message: ApiEphemeraMessage) -> Result<()> {
        let url = format!("{}/{}", self.url, "ephemera/broadcast/submit_message");
        let response = self.client.post(&url).json(&message).send().await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(Error::UnexpectedResponse {
                status: response.status(),
                body: response.text().await?,
            })
        }
    }

    ///Store Key Value pair in the DHT.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ephemera::ephemera_api::{ApiDhtStoreRequest, EphemeraHttpClient};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///    let client = EphemeraHttpClient::new("http://localhost:7000/".to_string());
    ///     let request = unimplemented!("See how to create a ApiDhtStoreRequest");
    ///     client.store_dht_request(request).await?;
    ///  Ok(())
    /// }
    pub async fn store_dht_request(&self, request: ApiDhtStoreRequest) -> Result<()> {
        let url = format!("{}/{}", self.url, "ephemera/dht/store");
        let response = self.client.post(&url).json(&request).send().await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(Error::UnexpectedResponse {
                status: response.status(),
                body: response.text().await?,
            })
        }
    }

    ///Store Key Value pair in the DHT.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ephemera::ephemera_api::{ApiDhtStoreRequest, EphemeraHttpClient};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///    let client = EphemeraHttpClient::new("http://localhost:7000/".to_string());
    ///    let key = &[1, 2, 3];
    ///    let value = &[4, 5, 6];
    ///    client.store_dht_key_value(key, value).await?;
    ///    Ok(())
    /// }
    pub async fn store_dht_key_value(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let request = ApiDhtStoreRequest::new(key, value);
        self.store_dht_request(request).await
    }

    pub async fn query_dht_key(&self, request: ApiDhtQueryRequest) -> Result<Option<ApiDhtQueryResponse>> {
        let url = format!("ephemera/dht/query/{}", request.key_encoded());
        self.query_optional(&url).await
    }

    async fn query_optional<T: for<'de> serde::Deserialize<'de>>(
        &self,
        path: &str,
    ) -> Result<Option<T>> {
        let url = format!("{}/{}", self.url, path);
        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let body = response.json::<T>().await?;
                    Ok(Some(body))
                } else if response.status() == reqwest::StatusCode::NOT_FOUND {
                    Ok(None)
                } else {
                    return Err(Error::UnexpectedResponse {
                        status: response.status(),
                        body: response.text().await?,
                    });
                }
            }
            Err(err) => Err(err.into()),
        }
    }

    async fn query<T: for<'de> serde::Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = format!("{}/{}", self.url, path);
        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let body = response.json::<T>().await?;
                    Ok(body)
                } else {
                    Err(Error::UnexpectedResponse {
                        status: response.status(),
                        body: response.text().await?,
                    })
                }
            }
            Err(err) => Err(err.into()),
        }
    }
}
