use std::sync::{Arc, Mutex};

use reqwest::{IntoUrl, StatusCode, Url};

use ephemera::crypto::Keypair;
use ephemera::ephemera_api;

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

    pub(crate) async fn send_message(&mut self, msg: ephemera_api::ApiEphemeraMessage) {
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

    pub(crate) async fn block_by_hash(&self, hash: String) -> Option<ephemera_api::ApiBlock> {
        let path = format!("{}{}{}", self.http, "ephemera/block/", hash);
        let client: reqwest::Client = Default::default();
        match client.get(&path).send().await {
            Ok(res) => {
                if res.status() == StatusCode::NOT_FOUND {
                    return None;
                }
                let block: ephemera_api::ApiBlock = res.json().await.unwrap();
                Some(block)
            }
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
        let path = format!("{}{}{}", self.http, "ephemera/block/certificates/", hash);
        println!("Requesting certificates for block {hash} from {path}",);
        let client: reqwest::Client = Default::default();
        match client.get(&path).send().await {
            Ok(res) => {
                if res.status() == StatusCode::NOT_FOUND {
                    return None;
                }
                let certificates: Vec<ephemera_api::ApiCertificate> = res.json().await.unwrap();
                Some(certificates)
            }
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
