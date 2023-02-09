use std::sync::{Arc, Mutex};

use reqwest::{IntoUrl, Url};

use ephemera::api::types::{ApiBlock, ApiKeypair, ApiSignedMessage};
use ephemera::utilities::CryptoApi;

use crate::Data;

pub(crate) struct SignedMessageClient {
    http: Url,
    pub(crate) data: Arc<Mutex<Data>>,
}

impl SignedMessageClient {
    pub(crate) fn new<U: IntoUrl>(url: U, data: Arc<Mutex<Data>>) -> SignedMessageClient {
        SignedMessageClient {
            http: url.into_url().unwrap(),
            data,
        }
    }

    pub(crate) async fn send_message(&mut self, msg: ApiSignedMessage) {
        let path = format!("{}{}", self.http, "ephemera/submit_message");
        let client: reqwest::Client = Default::default();
        match client.post(&path).json(&msg).send().await {
            Ok(_) => {
                self.data.lock().unwrap().sent_messages.push(msg);
            }
            Err(err) => {
                println!("Error sending message: {err:?}",);
            }
        }
    }

    pub(crate) async fn block_by_id(&self, id: String) -> Option<ApiBlock> {
        let path = format!("{}{}{}", self.http, "ephemera/block/", id);
        let client: reqwest::Client = Default::default();
        match client.get(&path).send().await {
            Ok(res) => {
                let block: ApiBlock = res.json().await.unwrap();
                Some(block)
            }
            Err(err) => {
                println!("Error sending message: {err:?}",);
                None
            }
        }
    }

    pub(crate) async fn signed_message(&self, keypair: ApiKeypair, label: String) -> ApiSignedMessage {
        let id = uuid::Uuid::new_v4().to_string();
        let signature =
            CryptoApi::sign_message(id.clone(), "Message".to_string(), keypair.private_key)
                .unwrap();
        ApiSignedMessage::new(id, "Message".to_string(), signature.into(), label)
    }
}
