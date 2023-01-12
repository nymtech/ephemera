///! A callback for BroadcastProtocol which signs message payloads.
///!
///! After a message reaches consensus('committed' call from BroadcastProtocol), it will pass the message and its signatures to a backend.
///! Current backend implementation just saves the signatures in a file.
///!
///! Signatures verification for a message is not done at the moment.
///!
use std::collections::HashMap;

use async_trait::async_trait;

use crate::backend::file_backend::FileBackend;
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

#[derive(Debug)]
pub struct SignaturesConsensusRequest {
    pub request_id: String,
    pub payload: Vec<u8>,
    pub signatures: HashMap<String, Signer>,
}

pub struct SigningBroadcastCallBack {
    pub keypair: Ed25519KeyPair,
    pub requests: HashMap<String, SignaturesConsensusRequest>,
    pub file_backend: Option<FileBackend>,
    pub ws_backend: Option<WsBackend>,
    pub ws_handle: Option<WsManagerHandle>,
    pub ws_listen_addr: Option<String>,
}

impl SigningBroadcastCallBack {
    pub fn new() -> SigningBroadcastCallBack {
        let keypair = Ed25519KeyPair::generate().unwrap(); //TODO
        SigningBroadcastCallBack {
            keypair,
            requests: HashMap::new(),
            file_backend: None,
            ws_backend: None,
            ws_handle: None,
            ws_listen_addr: None,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        if self.file_backend.is_none() && self.ws_backend.is_none() {
            return Err(anyhow::anyhow!("No backend set"));
        }
        if let Some(ws_listen_addr) = self.ws_listen_addr.clone() {
            let ws_handle = WsManager::start(ws_listen_addr).await.unwrap();
            let ws_backend = WsBackend::new(ws_handle);
            self.ws_backend = Some(ws_backend);
        }
        Ok(())
    }

    pub fn with_file_backend(mut self, signatures_file: String) -> Self {
        self.file_backend = Some(FileBackend::new(signatures_file));
        self
    }

    pub fn with_ws_backend(mut self, ws_listen_addr: String) -> Self {
        self.ws_listen_addr = Some(ws_listen_addr);
        self
    }
}

use anyhow::Result;
use ephemera::broadcast_protocol::websocket::wsmanager::{WsManager, WsManagerHandle};

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

        //Trust broadcast_protocol to remember which messages were already sent to callback
        if let Some(scr) = self.requests.remove(&state.id) {
            let cloned = scr.signatures.values().cloned();
            let signatures = cloned.collect::<Vec<Signer>>();

            if let Some(file_backend) = &mut self.file_backend {
                file_backend.store(&scr.payload, signatures.clone())?
            }

            if let Some(ws_backend) = &mut self.ws_backend {
                let sig_msg = WsSignaturesMsg {
                    request_id: scr.request_id,
                    payload: scr.payload,
                    signatures,
                };
                let msg = serde_json::to_vec(&sig_msg)?;
                ws_backend.send(msg).await?
            }
        }
        Ok(())
    }
}
