//! Brute force, simple Merkle proof implementation for messages from Block root hash
//! Useful for verifying that a message is included in a block.
//! Or to put it other way, to verify that a message is "valid" given a valid block.
//!
//TODO - store merkle tree in block
use crate::block::types::block::{BlockHash, RawBlock};
use crate::block::types::message::EphemeraRawMessage;
use crate::utilities::encode;
use crate::utilities::hash::blake2_256;

pub(crate) struct Merkle;

impl Merkle {
    /// Calculate the Merkle hash of a block.
    ///
    /// If block has no messages, the root hash is the hash of the block header.
    ///
    /// If block has messages, the root hash is the hash of the block header and the hashes of the messages
    /// using the Merkle tree algorithm.
    pub(crate) fn calculate_root_hash(raw_block: &RawBlock) -> anyhow::Result<BlockHash> {
        let header_bytes = encode(&raw_block.header)?;
        let header_hash = blake2_256(&header_bytes);
        if raw_block.signed_messages.is_empty() {
            return Ok(header_hash);
        }
        let mut message_hashes = vec![];
        for message in &raw_block.signed_messages {
            let raw_message: EphemeraRawMessage = message.clone().into();
            let raw_message_bytes = encode(&raw_message)?;
            let hash = blake2_256(&raw_message_bytes);
            message_hashes.push(hash);
        }

        let mut iter = message_hashes.into_iter();
        let mut message_root = vec![];
        loop {
            let mut new_hashes = vec![];
            let first = iter.next();
            let second = iter.next();

            match (first, second) {
                (Some(first), Some(second)) => {
                    let mut hash = first.to_vec();
                    hash.extend_from_slice(&second);
                    let hash = blake2_256(&hash);
                    new_hashes.push(hash);
                }
                (Some(first), None) => {
                    new_hashes.push(first);
                    break;
                }
                _ => break,
            }
            if new_hashes.len() == 1 {
                message_root = new_hashes[0].to_vec();
                break;
            }
        }

        let mut root_hash = header_hash.to_vec();
        root_hash.extend_from_slice(&message_root);
        let root_hash = blake2_256(&root_hash);

        Ok(root_hash)
    }

    //FIXME  - not a very useful Merkle verification implementation
    /// Verify that a message is in a block.
    ///
    /// The message is verified by checking that the message index is correct and that the root hash of the block
    /// is the same as the root hash of the given block(it could be Merkle Tree instead).
    ///
    /// Anyone who calls this function must trust that root_hash and raw_block are valid(as source of truth).
    pub(crate) fn verify_message_hash_in_block(
        root_hash: BlockHash,
        raw_block: &RawBlock,
        message: EphemeraRawMessage,
        message_index: usize,
    ) -> anyhow::Result<bool> {
        //As we don't have a merkle tree saved anywhere(yet), we just do a brute force check
        let mut correct_index = false;
        for (i, msg) in raw_block.signed_messages.clone().into_iter().enumerate() {
            let raw_mgs: EphemeraRawMessage = msg.into();
            if i == message_index && raw_mgs == message {
                correct_index = true;
            }
        }
        if !correct_index {
            return Ok(false);
        }

        Ok(Self::calculate_root_hash(raw_block)? == root_hash)
    }
}

#[cfg(test)]
mod tests {
    use crate::block::types::block::RawBlockHeader;
    use crate::block::types::message::{EphemeraMessage, EphemeraRawMessage};
    use crate::utilities::crypto::{PeerId, Signature};
    use crate::utilities::id::generate_ephemera_id;

    use super::*;

    #[test]
    fn test_merkle() {
        let peer_id = PeerId::random();
        let header = RawBlockHeader::new(peer_id, 0);
        let messages = vec![
            EphemeraMessage::new(
                EphemeraRawMessage::new(generate_ephemera_id(), vec![1, 2, 3]),
                Signature::default(),
                "".to_string(),
            ),
            EphemeraMessage::new(
                EphemeraRawMessage::new(generate_ephemera_id(), vec![4, 5, 6]),
                Signature::default(),
                "".to_string(),
            ),
            EphemeraMessage::new(
                EphemeraRawMessage::new(generate_ephemera_id(), vec![7, 8, 9]),
                Signature::default(),
                "".to_string(),
            ),
        ];
        let raw_block = RawBlock::new(header, messages.clone());
        let root_hash = Merkle::calculate_root_hash(&raw_block).unwrap();

        assert!(Merkle::verify_message_hash_in_block(
            root_hash.clone(),
            &raw_block,
            messages[0].clone().into(),
            0,
        )
        .unwrap());
        assert!(Merkle::verify_message_hash_in_block(
            root_hash.clone(),
            &raw_block,
            messages[1].clone().into(),
            1,
        )
        .unwrap());
        assert!(Merkle::verify_message_hash_in_block(
            root_hash.clone(),
            &raw_block,
            messages[2].clone().into(),
            2,
        )
        .unwrap());
    }
}
