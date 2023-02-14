use std::sync::Arc;

use crate::api::types::{ApiBlock, ApiRawMessage, ApiSignedMessage};
use crate::block::RawMessage;
use crate::utilities::crypto::libp2p2_crypto::Libp2pKeypair;
use crate::utilities::crypto::signer::Libp2pSigner;
use crate::utilities::Signer;

//Cosmos style ABCI application hook, excluded unnecessary methods
pub trait ApplicationHook {
    //ApiSignedMessage probably will be generalized to ApiTransaction
    fn check_tx(&self, tx: ApiSignedMessage) -> anyhow::Result<bool>;
    fn deliver_block(&self, _block: ApiBlock) -> anyhow::Result<()>;
}

pub struct SignatureVerificationApplicationHook {
    signer: Libp2pSigner,
}

impl SignatureVerificationApplicationHook {
    pub fn new(key_pair: Arc<Libp2pKeypair>) -> Self {
        let signer = Libp2pSigner::new(key_pair);
        Self { signer }
    }

    pub(crate) fn verify_message(&self, msg: ApiSignedMessage) -> anyhow::Result<()> {
        let signature = msg.signature.clone();
        let raw_message: ApiRawMessage = msg.into();
        match self
            .signer
            .verify::<RawMessage>(&raw_message.into(), &signature.into())
        {
            Ok(true) => Ok(()),
            Ok(false) => Err(anyhow::anyhow!("Invalid signature")),
            Err(err) => Err(anyhow::anyhow!(err)),
        }
    }
}

impl ApplicationHook for SignatureVerificationApplicationHook {
    fn check_tx(&self, tx: ApiSignedMessage) -> anyhow::Result<bool> {
        log::trace!("SignatureVerificationApplicationHook::check_tx");
        self.verify_message(tx)?;
        Ok(true)
    }

    fn deliver_block(&self, _block: ApiBlock) -> anyhow::Result<()> {
        log::trace!("SignatureVerificationApplicationHook::deliver_block");
        Ok(())
    }
}
