use thiserror::Error;

use crate::api::types::Health;
use crate::ephemera_api::{ApiBlock, ApiCertificate, ApiEphemeraMessage};

#[derive(Error, Debug)]
pub enum Error {
    #[error("Query failed")]
    Internal(#[from] reqwest::Error),
    #[error("Unexpected response: {status} {body}")]
    UnexpectedResponse {
        status: reqwest::StatusCode,
        body: String,
    },
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

pub struct EphemeraHttpClient {
    pub(crate) client: reqwest::Client,
    pub(crate) url: String,
}

impl EphemeraHttpClient {
    pub fn new(url: String) -> Self {
        let client = reqwest::Client::new();
        Self { client, url }
    }

    pub async fn health(&self) -> Result<Health> {
        self.query("ephemera/health").await
    }

    pub async fn get_block_by_hash(&self, hash: &str) -> Result<Option<ApiBlock>> {
        let url = format!("ephemera/block/{hash}",);
        self.query_optional(&url).await
    }

    pub async fn get_block_certificates(&self, hash: &str) -> Result<Option<Vec<ApiCertificate>>> {
        let url = format!("ephemera/block/certificates/{hash}",);
        self.query_optional(&url).await
    }

    pub async fn get_block_by_height(&self, height: u64) -> Result<Option<ApiBlock>> {
        let url = format!("ephemera/block/height/{height}",);
        self.query_optional(&url).await
    }

    pub async fn get_last_block(&self) -> Result<ApiBlock> {
        self.query("ephemera/blocks/last").await
    }

    pub async fn submit_message(&self, message: ApiEphemeraMessage) -> Result<()> {
        let url = format!("{}/{}", self.url, "ephemera/submit_message");
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
