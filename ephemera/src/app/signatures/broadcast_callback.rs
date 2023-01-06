///! A callback for BroadcastProtocol which signs message payloads.
///!
///! After a message reaches consensus('committed' call from BroadcastProtocol), it will pass the message and its signatures to a backend.
///! Current backend implementation just saves the signatures in a file.
///!
///! Signatures verification for a message is not done at the moment.
///!
use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::app::signatures::file_backend::SignaturesBackend;
use crate::broadcast_protocol::broadcast::ConsensusContext;
use crate::broadcast_protocol::BroadcastCallBack;
use crate::crypto::ed25519::{Ed25519KeyPair, KeyPair};
use crate::settings::Settings;

#[derive(Deserialize, Serialize)]
pub struct SignatureRequest {
    pub payload: Vec<u8>,
    pub signature: String,
}

#[derive(Debug, Clone)]
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
    pub backend: SignaturesBackend,
}

impl SigningBroadcastCallBack {
    pub fn new(settings: Settings) -> SigningBroadcastCallBack {
        let keypair = Ed25519KeyPair::generate().unwrap(); //TODO
        let backend = SignaturesBackend::new(settings);
        SigningBroadcastCallBack {
            keypair,
            requests: HashMap::new(),
            backend,
        }
    }
}

impl BroadcastCallBack for SigningBroadcastCallBack {
    fn pre_prepare(
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
    fn prepare(
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

        log::debug!(
            "SIG {}: {}, {}",
            ctx.original_sender,
            scr.signatures.contains_key(&sender),
            sender
        );
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
    fn committed(&mut self, state: &ConsensusContext) -> Result<()> {
        log::debug!("COMMITTED");

        //Trust broadcast_protocol to remember which messages were already sent to callback
        if let Some(scr) = self.requests.remove(&state.id) {
            let cloned = scr.signatures.values().cloned();
            let signatures = cloned.collect::<Vec<Signer>>();
            self.backend.store(&scr.payload, signatures)?
        }
        Ok(())
    }
}
