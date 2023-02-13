use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::sync::Arc;

use lru::LruCache;

use crate::block::{Block, RawBlock};
use crate::utilities::crypto::libp2p2_crypto::Libp2pKeypair;
use crate::utilities::crypto::Signature;
use crate::utilities::crypto::signer::Libp2pSigner;
use crate::utilities::Signer;

pub(crate) struct BlockSigner {
    /// All signatures of the last blocks that we received from the network(+ our own)
    recent_verified_block_signatures: LruCache<String, HashSet<Signature>>,
    signer: Libp2pSigner,
}

impl BlockSigner {
    pub fn new(key_pair: Arc<Libp2pKeypair>) -> Self {
        let signer = Libp2pSigner::new(key_pair);
        Self { recent_verified_block_signatures: LruCache::new(NonZeroUsize::new(1000).unwrap()), signer }
    }

    pub(crate) fn get_block_signatures(&mut self, block_id: &str) -> Option<Vec<Signature>> {
        self.recent_verified_block_signatures.get_mut(block_id).map(|signatures| signatures.iter().cloned().collect())
    }

    pub(crate) fn sign_block(&mut self, block: Block) -> anyhow::Result<Signature> {
        let id = block.header.id.clone();
        let raw_block: RawBlock = block.into();
        let signature = self.signer.sign(&raw_block)?;
        self.add_signature(&id, signature.clone());
        Ok(signature)
    }

    pub(crate) fn verify_block(
        &mut self,
        block: &Block,
        signature: &Signature,
    ) -> anyhow::Result<()> {
        if self.recent_verified_block_signatures.contains(&block.header.id) &&
            self.recent_verified_block_signatures.get(&block.header.id).unwrap().contains(signature) {
            return Ok(());
        }
        let raw_block: RawBlock = (*block).clone().into();
        match self.signer.verify(&raw_block, signature) {
            Ok(true) => {
                self.add_signature(&block.header.id, signature.clone());
                Ok(())
            }
            Ok(false) => Err(anyhow::anyhow!("Invalid signature")),
            Err(err) => Err(anyhow::anyhow!(err)),
        }
    }

    fn add_signature(&mut self, block_id: &str, signature: Signature) {
        self.recent_verified_block_signatures
            .get_or_insert_mut(block_id.to_owned(), HashSet::new)
            .insert(signature);
    }
}
