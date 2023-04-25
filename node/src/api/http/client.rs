use std::time::Duration;

use thiserror::Error;

use crate::api::types::{ApiBroadcastInfo, ApiHealth};
use crate::ephemera_api::{
    ApiBlock, ApiCertificate, ApiDhtQueryRequest, ApiDhtQueryResponse, ApiDhtStoreRequest,
    ApiEphemeraConfig, ApiEphemeraMessage,
};

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

/// Client to interact with the node over HTTP api.
pub struct Client {
    pub(crate) client: reqwest::Client,
    pub(crate) url: String,
}

impl Client {
    /// Create a http new client.
    ///
    /// # Arguments
    /// * `url` - The url of the node api endpoint.
    #[must_use]
    pub fn new(url: String) -> Self {
        let client = reqwest::Client::new();
        Self { client, url }
    }

    /// Create a new client.
    ///
    /// # Arguments
    /// * `url` - The url of the node api endpoint.
    /// * `timeout_sec` - Request timeout in seconds.
    ///
    /// # Panics
    /// If the client cannot be created.
    #[must_use]
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
    /// ```no_run
    /// use ephemera::ephemera_api::{Client, ApiHealth};
    ///
    /// #[tokio::main]
    ///async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///   let client = Client::new("http://localhost:7000".to_string());
    ///   let health = client.health().await.unwrap();
    ///    Ok(())
    /// }
    /// ```
    ///
    /// # Returns
    /// * [`ApiHealth`] - The health of the node.
    ///
    /// # Errors
    /// If the request fails.
    pub async fn health(&self) -> Result<ApiHealth> {
        self.query("ephemera/node/health").await
    }

    /// Get the block by hash.
    ///
    /// # Example
    /// ```no_run
    /// use ephemera::ephemera_api::{ApiBlock, Client};
    ///
    /// #[tokio::main]
    ///async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = Client::new("http://localhost:7000".to_string());
    ///     let block = client.get_block_by_hash("9D2LaY17rbnxfgKUbvcsJ5cB2BRHEd8fPJwsBnDHNGBX").await?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Returns
    /// * Option<[`ApiBlock`]> - The block.
    ///
    /// # Errors
    /// If the request fails.
    pub async fn get_block_by_hash(&self, hash: &str) -> Result<Option<ApiBlock>> {
        let url = format!("ephemera/broadcast/block/{hash}",);
        self.query_optional(&url).await
    }

    /// Get the block certificates by hash.
    ///
    /// # Example
    /// ```no_run
    /// use ephemera::ephemera_api::{ApiCertificate, Client};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///    let client = Client::new("http://localhost:7000".to_string());
    ///    let certificates = client.get_block_certificates("9D2LaY17rbnxfgKUbvcsJ5cB2BRHEd8fPJwsBnDHNGBX").await?;
    ///    Ok(())
    /// }
    /// ```
    ///
    /// # Arguments
    /// * `hash` - The hash of the block.
    ///
    /// # Returns
    /// * Option<Vec<[`ApiCertificate`]>> - The block certificates.
    ///
    /// # Errors
    /// If the request fails.
    pub async fn get_block_certificates(&self, hash: &str) -> Result<Option<Vec<ApiCertificate>>> {
        let url = format!("ephemera/broadcast/block/certificates/{hash}",);
        self.query_optional(&url).await
    }

    /// Get the block by height.
    ///
    /// # Example
    /// ```no_run
    /// use ephemera::ephemera_api::{ApiBlock, Client};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///   let client = Client::new("http://localhost:7000/".to_string());
    ///   let block = client.get_block_by_height(1).await?;
    ///   Ok(())
    /// }
    /// ```
    ///
    /// # Arguments
    /// * `height` - The height of the block.
    ///
    /// # Returns
    /// * Option<[`ApiBlock`]> - The block.
    ///
    /// # Errors
    /// If the request fails.
    pub async fn get_block_by_height(&self, height: u64) -> Result<Option<ApiBlock>> {
        let url = format!("ephemera/broadcast/block/height/{height}",);
        self.query_optional(&url).await
    }

    /// Get the last block.
    ///
    /// # Example
    /// ```no_run
    /// use ephemera::ephemera_api::{ApiBlock, Client};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///    let client = Client::new("http://localhost:7000/".to_string());
    ///    let block = client.get_last_block().await?;
    ///    Ok(())
    /// }
    /// ```
    ///
    /// # Returns
    /// * [`ApiBlock`] - The last block.
    ///
    /// # Errors
    /// If the request fails.
    pub async fn get_last_block(&self) -> Result<ApiBlock> {
        self.query("ephemera/broadcast/blocks/last").await
    }

    /// Get the node configuration.
    ///
    /// # Example
    /// ```no_run
    /// use ephemera::ephemera_api::{ApiEphemeraConfig, Client};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///    let client = Client::new("http://localhost:7000/".to_string());
    ///    let config = client.get_ephemera_config().await?;
    ///    Ok(())
    /// }
    /// ```
    ///
    /// # Returns
    /// * [`ApiEphemeraConfig`] - The node configuration.
    ///
    /// # Errors
    /// If the request fails.
    pub async fn get_ephemera_config(&self) -> Result<ApiEphemeraConfig> {
        self.query("ephemera/node/config").await
    }

    /// Submit a message to the node.
    ///
    /// # Example
    /// ```no_run
    /// use ephemera::ephemera_api::{ApiEphemeraMessage, Client};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///   let client = Client::new("http://localhost:7000/".to_string());
    ///   let message = unimplemented!("See how to create a ApiEphemeraMessage");
    ///   client.submit_message(message).await?;
    ///   Ok(())
    /// }
    ///
    /// ```
    ///
    /// # Arguments
    /// * `message` - The message to submit.
    ///
    /// # Errors
    /// If the request fails.
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
    ///```no_run
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///    use ephemera::ephemera_api::Client;
    ///    let client = Client::new("http://localhost:7000/".to_string());
    ///    let request = unimplemented!("See how to create a ApiDhtStoreRequest");
    ///    client.store_in_dht(request).await?;
    ///  Ok(())
    /// }
    /// ```
    ///
    /// # Arguments
    /// * `request` - Key Value pair to store.
    ///
    /// # Errors
    /// If the request fails.
    pub async fn store_in_dht(&self, request: ApiDhtStoreRequest) -> Result<()> {
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
    ///```no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///    use ephemera::ephemera_api::Client;
    ///    let client = Client::new("http://localhost:7000/".to_string());
    ///    let key = &[1, 2, 3];
    ///    let value = &[4, 5, 6];
    ///    client.store_in_dht_key_value(key, value).await?;
    ///    Ok(())
    /// }
    /// ```
    /// # Arguments
    /// * `key` - Key to use to store the value.
    /// * `value` - Value to store.
    ///
    /// # Errors
    /// If the request fails.
    pub async fn store_in_dht_key_value(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let request = ApiDhtStoreRequest::new(key, value);
        self.store_in_dht(request).await
    }

    /// Query the DHT for a given key.
    ///
    /// # Example
    ///```no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///    use ephemera::ephemera_api::{ApiDhtQueryRequest, Client};
    ///    let client = Client::new("http://localhost:7000/".to_string());
    ///    let request = ApiDhtQueryRequest::new(&[1, 2, 3]);
    ///    let response = client.query_dht(request).await?;
    ///    Ok(())
    /// }
    /// ```
    ///
    /// # Arguments
    /// * `request` - Key to query.
    ///
    /// # Returns
    /// * Option<[`ApiDhtQueryResponse`]> - The value stored in the DHT for the given key.
    ///
    /// # Errors
    /// If the request fails.
    pub async fn query_dht(
        &self,
        request: ApiDhtQueryRequest,
    ) -> Result<Option<ApiDhtQueryResponse>> {
        let url = format!("ephemera/dht/query/{}", request.key_encoded());
        self.query_optional(&url).await
    }

    /// Query the DHT for a given key.
    ///
    /// # Example
    /// ```no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///   use ephemera::ephemera_api::Client;
    ///   let client = Client::new("http://localhost:7000/".to_string());
    ///   let key = &[1, 2, 3];
    ///   let response = client.query_dht_key(key).await?;
    ///   Ok(())
    /// }
    /// ```
    ///
    /// # Arguments
    /// * `key` - Key to query.
    ///
    /// # Returns
    /// * Option<[`ApiDhtQueryResponse`]> - The value stored in the DHT for the given key.
    ///
    /// # Errors
    /// If the request fails.
    pub async fn query_dht_key(&self, key: &[u8]) -> Result<Option<ApiDhtQueryResponse>> {
        let request = ApiDhtQueryRequest::new(key);
        self.query_dht(request).await
    }

    /// Get broadcast group info.
    ///
    /// # Example
    /// ```no_run
    /// use ephemera::ephemera_api::Client;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///   let client = Client::new("http://localhost:7000/".to_string());
    ///   let info = client.broadcast_info().await?;
    ///   Ok(())
    /// }
    /// ```
    ///
    /// # Returns
    /// * [`ApiBroadcastInfo`] - The broadcast group info.
    ///
    /// # Errors
    /// If the request fails.
    pub async fn broadcast_info(&self) -> Result<ApiBroadcastInfo> {
        self.query("ephemera/broadcast/group/info").await
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
