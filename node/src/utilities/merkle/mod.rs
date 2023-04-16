use crate::block::types::block::Block;
use crate::block::types::message::EphemeraMessage;
use crate::codec::Encode;
use crate::utilities::hash::{EphemeraHasher, HashType, Hasher};
use log::info;

pub(crate) struct Merkle;

impl Merkle {
    /// Calculate block merkle root hash
    pub(crate) fn calculate_merkle_root<H: EphemeraHasher>(
        block: &Block,
        mut hasher: H,
    ) -> anyhow::Result<HashType> {
        let header_bytes = block.header.encode()?;
        hasher.update(&header_bytes);

        if block.messages.is_empty() {
            let hash = hasher.finish().into();
            return Ok(hash);
        }
        let mut message_hashes = vec![];
        for message in &block.messages {
            let bytes = message.encode()?;
            let hash = Hasher::digest(bytes.as_slice());
            message_hashes.push(hash);
        }

        let mut iter = message_hashes.into_iter();
        let mut messages_root = vec![];
        loop {
            let mut new_hashes = vec![];
            let first = iter.next();
            let second = iter.next();

            match (first, second) {
                (Some(first), Some(second)) => {
                    let mut hash = first.to_vec();
                    hash.extend_from_slice(&second);
                    let hash = Hasher::digest(&hash);
                    new_hashes.push(hash);
                }
                (Some(first), None) => {
                    new_hashes.push(first);
                    break;
                }
                (None, None) => {}
                _ => break,
            }

            if new_hashes.len() == 1 {
                messages_root = new_hashes[0].to_vec();
                break;
            } else if new_hashes.len() == 2 {
                let mut hash = new_hashes[0].to_vec();
                hash.extend_from_slice(&new_hashes[1]);
                messages_root = Hasher::digest(&hash).to_vec();
                break;
            } else {
                info!("Merkle tree has {} hashes", new_hashes.len());
                iter = new_hashes.into_iter();
            }
        }

        let mut header_hash = hasher.finish().to_vec();
        header_hash.extend_from_slice(&messages_root);
        let root_hash = Hasher::digest(&header_hash);

        Ok(root_hash.into())
    }

    /// Verify that a message is in a block.
    /// The message is verified by checking that the merkle_root of the block matches the given merkle_root.
    ///
    /// PS! It doesn't check message index
    pub(crate) fn verify_message_hash_in_block(
        merkle_root: HashType,
        block: &Block,
        message: EphemeraMessage,
    ) -> anyhow::Result<bool> {
        let mut correct_index = false;
        for msg in block.messages.clone().into_iter() {
            if msg == message {
                correct_index = true;
            }
        }
        if !correct_index {
            return Ok(false);
        }

        //As we don't have a merkle tree saved anywhere(yet), we just do a brute force check
        let hasher = Hasher::default();
        Ok(Self::calculate_merkle_root(block, hasher)? == merkle_root)
    }
}

#[cfg(test)]
mod tests {
    use crate::crypto::EphemeraKeypair;
    use crate::{
        block::{
            types::block::{Block, RawBlock, RawBlockHeader},
            types::message::{EphemeraMessage, RawEphemeraMessage},
        },
        codec::Encode,
        crypto::Keypair,
        network::peer::PeerId,
        utilities::{crypto::Certificate, hash::Hasher, merkle::Merkle},
    };

    #[test]
    fn test_merkle() {
        for i in 0..=10 {
            let (block, messages) = new_block(i);
            let merkle_root = Merkle::calculate_merkle_root(&block, Hasher::default()).unwrap();

            for m in 0..i {
                assert!(Merkle::verify_message_hash_in_block(
                    merkle_root.clone(),
                    &block,
                    messages[m].clone().into(),
                )
                .unwrap());
            }
        }
    }

    fn new_block(nr_of_messages: usize) -> (Block, Vec<EphemeraMessage>) {
        let peer_id = PeerId::random();
        let header = RawBlockHeader::new(peer_id, 0);

        let keypair = Keypair::generate(None);
        let signature = keypair.sign(&header.encode().unwrap()).unwrap();
        let certificate = Certificate::new(signature, keypair.public_key());

        let mut messages = vec![];
        for i in 0..nr_of_messages {
            messages.push(EphemeraMessage::new(
                RawEphemeraMessage::new(format!("label{}", i), vec![i as u8]),
                certificate.clone(),
            ));
        }

        let raw_block = RawBlock::new(header.clone(), messages.clone());
        let block_hash = raw_block.hash_with_default_hasher().unwrap();
        let block = Block::new(raw_block, block_hash);
        (block, messages)
    }
}
