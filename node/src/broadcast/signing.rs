use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::sync::Arc;

use lru::LruCache;

use crate::block::types::block::{Block, RawBlock};
use crate::utilities::crypto::ed25519::Ed25519Keypair;
use crate::utilities::crypto::Signature;
use crate::utilities::crypto::{Keypair, PublicKey};
use crate::utilities::encoding::Encode;
use crate::utilities::Ed25519PublicKey;

pub(crate) struct BlockSigner {
    /// All signatures of the last blocks that we received from the network(+ our own)
    recent_verified_block_signatures: LruCache<String, HashSet<Signature>>,
    keypair: Arc<Ed25519Keypair>,
}

impl BlockSigner {
    pub fn new(keypair: Arc<Ed25519Keypair>) -> Self {
        Self {
            recent_verified_block_signatures: LruCache::new(NonZeroUsize::new(1000).unwrap()),
            keypair,
        }
    }

    pub(crate) fn get_block_signatures(&mut self, block_id: &str) -> Option<Vec<Signature>> {
        self.recent_verified_block_signatures
            .get_mut(block_id)
            .map(|signatures| signatures.iter().cloned().collect())
    }

    pub(crate) fn sign_block(&mut self, block: Block) -> anyhow::Result<Signature> {
        let id = block.header.id.clone();

        let raw_block: RawBlock = block.into();
        let encoded_block = raw_block.encode()?;

        let signature = self.keypair.sign(&encoded_block)?;
        let pub_key = self.keypair.public_key().to_raw_vec();

        let signature = Signature::new(signature, pub_key);

        self.add_signature(&id, signature.clone());

        Ok(signature)
    }

    /// This verification is part of reliable broadcast and verifies only the
    /// signature of the sender.
    pub(crate) fn verify_block(
        &mut self,
        block: &Block,
        signature: &Signature,
    ) -> anyhow::Result<()> {
        if self
            .recent_verified_block_signatures
            .get(&block.header.id)
            .map(|signatures| signatures.contains(signature))
            .unwrap_or(false)
        {
            return Ok(());
        }

        let raw_block: RawBlock = (*block).clone().into();
        let encoded_block = raw_block.encode()?;

        let public_key = Ed25519PublicKey::from_raw_vec(signature.public_key.clone())?;

        if public_key.verify(&encoded_block, &signature.signature) {
            self.add_signature(&block.header.id, signature.clone());
            Ok(())
        } else {
            anyhow::bail!("Invalid block signature");
        }
    }

    fn add_signature(&mut self, block_id: &str, signature: Signature) {
        self.recent_verified_block_signatures
            .get_or_insert_mut(block_id.to_owned(), HashSet::new)
            .insert(signature);
    }
}
