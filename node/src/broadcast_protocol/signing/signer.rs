///! A callback for BroadcastProtocol which signs message payloads.
///!
///! After a message reaches consensus('committed' call from BroadcastProtocol), it will pass the message and its signatures to a backend.
///! Current backend implementation just saves the signatures in a file.
///!
///! Signatures verification for a message is not done at the moment.
///!
use std::collections::HashMap;

use async_trait::async_trait;

use serde::{Deserialize, Serialize};

use crate::broadcast_protocol::backend::websocket::ws_backend::{WsBackend, WsSignaturesMsg};
use crate::broadcast_protocol::backend::websocket::wsmanager::WsManager;
use crate::broadcast_protocol::broadcast::ConsensusContext;
use crate::broadcast_protocol::BroadcastCallBack;
use crate::config::configuration::{DbConfig, WsConfig};
use crate::crypto::ed25519::Ed25519KeyPair;
use crate::crypto::KeyPair;
use anyhow::Result;
use crate::database::store::{DbBackendHandle, DbStore};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SignatureRequest {
    pub payload: Vec<u8>,
    pub signature: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Signature {
    pub id: String,
    pub signature: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SignedConsensusMessage {
    pub request_id: String,
    pub message: Vec<u8>,
    pub signatures: HashMap<String, Signature>,
}

pub struct Signer {
    pub keypair: Ed25519KeyPair,
    pub ws_backend: WsBackend,
    pub db_backend: DbBackendHandle,
    pub requests: HashMap<String, SignedConsensusMessage>,
}

impl Signer {
    pub async fn start(
        keypair: Ed25519KeyPair,
        db_settings: DbConfig,
        ws_settings: WsConfig,
    ) -> Result<Signer> {
        let (ws_handle, _) = WsManager::start(ws_settings.ws_address).await.unwrap();
        let ws_backend = WsBackend::new(ws_handle);

        let (db_backend, _) = DbStore::start(db_settings.db_path).await?;
        let signer = Signer {
            keypair,
            ws_backend,
            db_backend,
            requests: HashMap::new(),
        };
        Ok(signer)
    }
}

#[async_trait]
impl BroadcastCallBack for Signer {
    async fn pre_prepare(
        &mut self,
        msg_id: String,
        _sender: String,
        message: Vec<u8>,
        ctx: &ConsensusContext,
    ) -> Result<Option<Vec<u8>>> {
        log::debug!("PRE_PREPARE");

        if self.requests.contains_key(&msg_id) {
            log::error!("Unexpected pre-prepare message {}", msg_id);
            return Ok(None);
        }

        let mut scr = SignedConsensusMessage {
            request_id: msg_id.clone(),
            message: message.clone(),
            signatures: HashMap::new(),
        };

        let signature = self.keypair.sign_hex(message.as_slice())?;

        let signer = Signature {
            id: ctx.local_address.clone(),
            signature: signature.clone(),
        };
        scr.signatures.insert(ctx.local_address.clone(), signer);
        self.requests.insert(msg_id, scr);

        let result = serde_json::to_vec(&SignatureRequest {
            payload: message,
            signature,
        })?;

        Ok(Some(result))
    }
    async fn prepare(
        &mut self,
        msg_id: String,
        sender: String,
        payload: Vec<u8>,
        ctx: &ConsensusContext,
    ) -> Result<Option<Vec<u8>>> {
        log::debug!("PREPARE");

        let sig_req: SignatureRequest = serde_json::from_slice(&payload)?;

        let scr = self
            .requests
            .entry(msg_id.clone())
            .or_insert_with(|| SignedConsensusMessage {
                request_id: msg_id.clone(),
                message: sig_req.payload.clone(),
                signatures: HashMap::new(),
            });

        let signer = Signature {
            id: sender.clone(),
            signature: sig_req.signature.clone(),
        };
        scr.signatures.insert(sender.clone(), signer);

        if !ctx.original_sender && !scr.signatures.contains_key(&ctx.local_address) {
            let payload = sig_req.payload.clone();
            let signature = self.keypair.sign_hex(payload.as_slice())?;

            let signer = Signature {
                id: ctx.local_address.clone(),
                signature: signature.clone(),
            };
            scr.signatures.insert(ctx.local_address.clone(), signer);

            let result = serde_json::to_vec(&SignatureRequest { payload, signature })?;

            return Ok(Some(result));
        }
        Ok(None)
    }
    async fn committed(&mut self, state: &ConsensusContext) -> Result<()> {
        log::debug!("COMMITTED");

        if let Some(scr) = self.requests.remove(&state.id) {
            self.db_backend.store(scr.clone()).await?;

            let sig_msg = WsSignaturesMsg { request: scr.clone() };
            let msg = serde_json::to_vec(&sig_msg)?;
            self.ws_backend.send(msg).await?
        }
        Ok(())
    }
}
