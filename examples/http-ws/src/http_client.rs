use std::sync::{Arc, Mutex};

use reqwest::{IntoUrl, StatusCode, Url};

use ephemera::api::types;
use ephemera::codec::Encode;
use ephemera::crypto::{Ed25519Keypair, EphemeraKeypair};
use ephemera::id::EphemeraId;

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

    pub(crate) async fn send_message(&mut self, msg: types::ApiEphemeraMessage) {
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

    pub(crate) async fn block_by_id(&self, id: EphemeraId) -> Option<types::ApiBlock> {
        let path = format!("{}{}{}", self.http, "ephemera/block/", id);
        let client: reqwest::Client = Default::default();
        match client.get(&path).send().await {
            Ok(res) => {
                if res.status() == StatusCode::NOT_FOUND {
                    return None;
                }
                let block: types::ApiBlock = res.json().await.unwrap();
                Some(block)
            }
            Err(err) => {
                println!("Error sending message: {err:?}",);
                None
            }
        }
    }

    pub(crate) async fn signed_message(
        &self,
        keypair: Arc<Ed25519Keypair>,
        label: String,
    ) -> types::ApiEphemeraMessage {
        let api_raw_message =
            types::RawApiEphemeraMessage::new("Label".to_string(), "Message".as_bytes().to_vec());

        let api_raw_message = api_raw_message.encode().unwrap();

        //FIXME - decide what to sign, encoded message or its hash
        let signature = keypair.sign(&api_raw_message).unwrap();
        let api_signature = types::ApiCertificate::new(signature, keypair.public_key());

        types::ApiEphemeraMessage::new("Message".as_bytes().to_vec(), api_signature, label)
    }
}
