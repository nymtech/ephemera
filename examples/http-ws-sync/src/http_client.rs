use std::sync::{Arc, Mutex};

use ephemera::crypto::Keypair;
use ephemera::ephemera_api;
use ephemera::ephemera_api::Client;

use crate::Data;

pub(crate) struct SignedMessageClient {
    client: Client,
    pub(crate) data: Arc<Mutex<Data>>,
}

impl SignedMessageClient {
    pub(crate) fn new(url: String, data: Arc<Mutex<Data>>) -> SignedMessageClient {
        let client = Client::new(url);
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
        hash: &str,
    ) -> Option<Vec<ephemera_api::ApiCertificate>> {
        match self.client.get_block_certificates(hash).await {
            Ok(certificates) => certificates,
            Err(err) => {
                println!("Error sending message: {err:?}",);
                None
            }
        }
    }

    pub(crate) async fn block_broadcast_info(
        &self,
        hash: &str,
    ) -> Option<ephemera_api::ApiBlockBroadcastInfo> {
        match self.client.get_block_broadcast_info(hash).await {
            Ok(info) => info,
            Err(err) => {
                println!("Error sending message: {err:?}",);
                None
            }
        }
    }

    pub(crate) async fn verify_message(
        &self,
        block_hash: &str,
        message_hash: &str,
        index: usize,
    ) -> anyhow::Result<bool> {
        self.client
            .verify_message_in_block(block_hash, message_hash, index)
            .await
            .map_err(|e| e.into())
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
