///! A callback for BroadcastProtocol which signs message payloads.
///!
///! After a message reaches consensus('committed' call from BroadcastProtocol), it will pass the message and its signatures to a backend.
///! Current backend implementation just saves the signatures in a file.
///!
///! Signatures verification for a message is not done at the moment.
///!
use std::collections::HashMap;

use async_trait::async_trait;

use crate::backend::file_backend::{FileBackendHandle, SignaturesFileBackend};
use crate::backend::websocket_backend::{WsBackend, WsSignaturesMsg};
use ephemera::broadcast_protocol::broadcast::ConsensusContext;
use ephemera::broadcast_protocol::BroadcastCallBack;
use ephemera::crypto::ed25519::Ed25519KeyPair;
use ephemera::crypto::KeyPair;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SignatureRequest {
    pub payload: Vec<u8>,
    pub signature: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Signer {
    pub id: String,
    pub signature: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SignaturesConsensusRequest {
    pub request_id: String,
    pub payload: Vec<u8>,
    pub signatures: HashMap<String, Signer>,
}

pub struct SigningBroadcastCallBack {
    pub keypair: Ed25519KeyPair,
    pub requests: HashMap<String, SignaturesConsensusRequest>,
    pub file_backend: Option<FileBackendHandle>,
    pub file_backend_file: Option<String>,
    pub ws_backend: Option<WsBackend>,
    pub ws_listen_addr: Option<String>,
    pub db_backend: Option<DbBackendHandle>,
    pub db_url: Option<String>,
}

impl SigningBroadcastCallBack {
    pub fn new() -> SigningBroadcastCallBack {
        let keypair = Ed25519KeyPair::generate().unwrap(); //TODO
        SigningBroadcastCallBack {
            keypair,
            requests: HashMap::new(),
            file_backend: None,
            file_backend_file: None,
            ws_backend: None,
            ws_listen_addr: None,
            db_backend: None,
            db_url: None,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        if let Some(file_backend_file) = &self.file_backend_file {
            let (file_backend_handle, _) = SignaturesFileBackend::start(file_backend_file.clone())?;
            self.file_backend = Some(file_backend_handle);
        }
        if let Some(ws_listen_addr) = self.ws_listen_addr.clone() {
            let (ws_handle, _) = WsManager::start(ws_listen_addr).await.unwrap();
            let ws_backend = WsBackend::new(ws_handle);
            self.ws_backend = Some(ws_backend);
        }
        if let Some(db_url) = self.db_url.clone() {
            let (db_backend, _) = DbBackend::start(db_url).await?;
            self.db_backend = Some(db_backend);
        }
        Ok(())
    }

    pub fn with_file_backend(mut self, file_backend_file: String) -> Self {
        self.file_backend_file = Some(file_backend_file);
        self
    }

    pub fn with_ws_backend(mut self, ws_listen_addr: String) -> Self {
        self.ws_listen_addr = Some(ws_listen_addr);
        self
    }

    pub fn with_db_backend(mut self, db_url: String) -> Self {
        self.db_url = Some(db_url);
        self
    }
}

use crate::backend::db_backend::{DbBackend, DbBackendHandle};
use anyhow::Result;
use ephemera::broadcast_protocol::websocket::wsmanager::WsManager;

#[async_trait]
impl BroadcastCallBack for SigningBroadcastCallBack {
    async fn pre_prepare(
        &mut self,
        msg_id: String,
        _sender: String,
        payload: Vec<u8>,
        ctx: &ConsensusContext,
    ) -> Result<Option<Vec<u8>>> {
        log::debug!("PRE_PREPARE");

        if self.requests.contains_key(&msg_id) {
            log::error!("Unexpected pre-prepare message {}", msg_id);
            return Ok(None);
        }

        let mut scr = SignaturesConsensusRequest {
            request_id: msg_id.clone(),
            payload: payload.clone(),
            signatures: HashMap::new(),
        };

        let signature = self.keypair.sign_hex(payload.as_slice())?;

        let signer = Signer {
            id: ctx.local_address.clone(),
            signature: signature.clone(),
        };
        scr.signatures.insert(ctx.local_address.clone(), signer);
        self.requests.insert(msg_id, scr);

        let result = serde_json::to_vec(&SignatureRequest { payload, signature })?;

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

        let scr =
            self.requests
                .entry(msg_id.clone())
                .or_insert_with(|| SignaturesConsensusRequest {
                    request_id: msg_id.clone(),
                    payload: sig_req.payload.clone(),
                    signatures: HashMap::new(),
                });

        let signer = Signer {
            id: sender.clone(),
            signature: sig_req.signature.clone(),
        };
        scr.signatures.insert(sender.clone(), signer);

        if !ctx.original_sender && !scr.signatures.contains_key(&ctx.local_address) {
            let payload = sig_req.payload.clone();
            let signature = self.keypair.sign_hex(payload.as_slice())?;

            let signer = Signer {
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
            if let Some(db_backend) = &mut self.db_backend {
                db_backend.store(scr.clone()).await?
            }

            if let Some(file_backend) = &mut self.file_backend {
                file_backend.store(scr.clone()).await?
            }

            if let Some(ws_backend) = &mut self.ws_backend {
                let sig_msg = WsSignaturesMsg {
                    request: scr.clone(),
                };
                let msg = serde_json::to_vec(&sig_msg)?;
                ws_backend.send(msg).await?
            }
        }
        Ok(())
    }
}
