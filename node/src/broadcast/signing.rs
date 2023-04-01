use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::sync::Arc;

use lru::LruCache;

use crate::block::types::block::{Block, RawBlock};
use crate::crypto::Keypair;
use crate::utilities::crypto::ed25519::Ed25519Keypair;
use crate::utilities::crypto::Certificate;
use crate::utilities::crypto::EphemeraPublicKey;
use crate::utilities::encoding::Encode;
use crate::utilities::hash::HashType;

pub(crate) struct BlockSigner {
    /// All signatures of the last blocks that we received from the network(+ our own)
    verified_signatures: LruCache<HashType, HashSet<Certificate>>,
    /// Our own keypair
    signing_keypair: Arc<Keypair>,
}

impl BlockSigner {
    pub fn new(keypair: Arc<Ed25519Keypair>) -> Self {
        Self {
            verified_signatures: LruCache::new(NonZeroUsize::new(1000).unwrap()),
            signing_keypair: keypair,
        }
    }

    pub(crate) fn get_block_signatures(&mut self, block_id: &HashType) -> Option<Vec<Certificate>> {
        self.verified_signatures
            .get(block_id)
            .map(|signatures| signatures.iter().cloned().collect())
    }

    pub(crate) fn sign_block(
        &mut self,
        block: &Block,
        hash: &HashType,
    ) -> anyhow::Result<Certificate> {
        let certificate = block.sign(self.signing_keypair.as_ref())?;
        self.add_certificate(hash, certificate.clone());
        Ok(certificate)
    }

    /// This verification is part of reliable broadcast and verifies only the
    /// signature of the sender.
    pub(crate) fn verify_block(
        &mut self,
        block: &Block,
        certificate: &Certificate,
    ) -> anyhow::Result<()> {
        if self
            .verified_signatures
            .get(&block.header.hash)
            .map(|signatures| signatures.contains(certificate))
            .unwrap_or(false)
        {
            log::trace!("Block already verified: {}", block.header.hash);
            return Ok(());
        }

        let raw_block: RawBlock = (*block).clone().into();
        let raw_block = raw_block.encode()?;

        if certificate
            .public_key
            .verify(&raw_block, &certificate.signature)
        {
            self.add_certificate(&block.header.hash, certificate.clone());
            Ok(())
        } else {
            anyhow::bail!("Invalid block signature");
        }
    }

    fn add_certificate(&mut self, block_id: &HashType, signature: Certificate) {
        self.verified_signatures
            .get_or_insert_mut(block_id.to_owned(), HashSet::new)
            .insert(signature);
    }
}

#[cfg(test)]
mod test {
    use crate::block::types::block::RawBlockHeader;
    use crate::block::types::message::{EphemeraMessage, UnsignedEphemeraMessage};
    use crate::network::peer::ToPeerId;
    use crate::utilities::crypto::ed25519::Ed25519Keypair;
    use crate::utilities::EphemeraKeypair;

    use super::*;

    #[test]
    fn test_sign_verify_block_ok() {
        let mut signer = BlockSigner::new(Arc::new(Keypair::generate(None)).clone());

        let message_signing_keypair = Keypair::generate(None);

        let block = new_block(&message_signing_keypair);
        let certificate = block.sign(&message_signing_keypair).unwrap();

        assert!(signer.verify_block(&block, &certificate).is_ok());
    }

    #[test]
    fn test_sign_verify_block_fail() {
        let mut signer = BlockSigner::new(Arc::new(Keypair::generate(None)).clone());
        let message_signing_keypair = Keypair::generate(None);

        let block = new_block(&message_signing_keypair);
        let certificate = block.sign(&message_signing_keypair).unwrap();

        let modified_block = new_block(&message_signing_keypair);

        assert!(signer.verify_block(&modified_block, &certificate).is_err());
    }

    fn new_block(keypair: &Ed25519Keypair) -> Block {
        let peer_id = keypair.public_key().peer_id();

        let raw_ephemera_message =
            UnsignedEphemeraMessage::new("label".to_string(), "payload".as_bytes().to_vec());

        let message_certificate = Certificate::prepare(keypair, &raw_ephemera_message).unwrap();
        let messages = vec![EphemeraMessage::new(
            raw_ephemera_message,
            message_certificate,
        )];

        let raw_block_header = RawBlockHeader::new(peer_id, 0);
        let raw_block = RawBlock::new(raw_block_header, messages);

        let block_hash = raw_block
            .hash_with_default_hasher()
            .expect("Hashing failed");

        let block = Block::new(raw_block, block_hash);
        block
    }
}
