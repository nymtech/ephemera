use std::sync::{Arc, Mutex};

use reqwest::{IntoUrl, StatusCode, Url};

use ephemera::crypto::Keypair;
use ephemera::ephemera_api;
use ephemera::ephemera_api::EphemeraHttpClient;

use crate::Data;

pub(crate) struct SignedMessageClient {
    client: EphemeraHttpClient,
    pub(crate) data: Arc<Mutex<Data>>,
}

impl SignedMessageClient {
    pub(crate) fn new(url: String, data: Arc<Mutex<Data>>) -> SignedMessageClient {
        let client = EphemeraHttpClient::new(url);
        SignedMessageClient { client, data }
    }

    pub(crate) async fn send_message(&mut self, msg: ephemera_api::ApiEphemeraMessage) {
        self.client.submit_message(msg).await.unwrap();
    }

    pub(crate) async fn block_by_hash(&self, hash: String) -> Option<ephemera_api::ApiBlock> {
        match self.client.get_block_by_hash(&hash).await {
            Ok(block) => block,
            Err(err) => {
                println!("Error sending message: {err:?}",);
                None
            }
        }
    }

    pub(crate) async fn block_certificates(
        &self,
        hash: String,
    ) -> Option<Vec<ephemera_api::ApiCertificate>> {
        match self.client.get_block_certificates(&hash).await {
            Ok(certificates) => certificates,
            Err(err) => {
                println!("Error sending message: {err:?}",);
                None
            }
        }
    }

    pub(crate) async fn signed_message(
        &self,
        keypair: Arc<Keypair>,
        label: String,
    ) -> ephemera_api::ApiEphemeraMessage {
        let raw_message =
            ephemera_api::RawApiEphemeraMessage::new(label, "Message".as_bytes().to_vec());
        let certificate = ephemera_api::ApiCertificate::prepare(&keypair, &raw_message).unwrap();

        ephemera_api::ApiEphemeraMessage::new(raw_message, certificate)
    }
}
