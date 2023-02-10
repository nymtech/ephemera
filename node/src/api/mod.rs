use std::sync::Arc;

use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::Mutex;

use crate::api::types::{ApiBlock, ApiKeypair, ApiSignature, ApiSignedMessage};
use crate::database::EphemeraDatabase;
use crate::ephemera::EphemeraDatabaseType;
use crate::utilities::crypto::signer::CryptoApi;
use crate::utilities::crypto::KeyPairError;

pub mod types;

#[derive(Debug, Clone)]
pub(crate) enum ApiCmd {
    SubmitSignedMessageRequest(ApiSignedMessage),
}

pub(crate) struct ApiListener {
    pub(crate) signed_messages_rcv: Receiver<ApiCmd>,
}

impl ApiListener {
    pub(crate) fn new(signed_messages_rcv: Receiver<ApiCmd>) -> Self {
        Self {
            signed_messages_rcv,
        }
    }
}

#[derive(Clone)]
pub struct EphemeraExternalApi {
    pub(crate) database: Arc<Mutex<EphemeraDatabaseType>>,
    pub(crate) submit_signed_messages_tx: Sender<ApiCmd>,
}

impl EphemeraExternalApi {
    pub(crate) fn new(
        database: Arc<Mutex<EphemeraDatabaseType>>,
    ) -> (EphemeraExternalApi, ApiListener) {
        let (submit_signed_messages_tx, signed_messages_rcv) = channel(100);

        let api_listener = ApiListener::new(signed_messages_rcv);

        let api = EphemeraExternalApi {
            database,
            submit_signed_messages_tx,
        };
        (api, api_listener)
    }

    pub async fn get_block_by_id(&self, block_id: String) -> anyhow::Result<Option<ApiBlock>> {
        let database = self.database.lock().await;
        let db_block = database
            .get_block_by_id(block_id)?
            .map(|block| block.into());
        Ok(db_block)
    }

    pub async fn get_block_by_label(&self, label: &str) -> anyhow::Result<Option<ApiBlock>> {
        let database = self.database.lock().await;
        let db_block = database
            .get_block_by_label(label)?
            .map(|block| block.into());
        Ok(db_block)
    }

    pub async fn submit_signed_message(&self, message: ApiSignedMessage) -> anyhow::Result<()> {
        let message = ApiCmd::SubmitSignedMessageRequest(message);
        self.submit_signed_messages_tx.send(message).await?;
        Ok(())
    }

    pub fn sign_message(
        &self,
        request_id: String,
        data: String,
        private_key: String,
    ) -> Result<ApiSignature, KeyPairError> {
        let signature = CryptoApi::sign_message(request_id, data, private_key)?;
        Ok(signature.into())
    }

    pub fn generate_key_pair(&self) -> Result<ApiKeypair, KeyPairError> {
        CryptoApi::generate_keypair()
    }
}
