use std::fmt::Display;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

use crate::block::types::message::EphemeraMessage;
use crate::network::PeerId;
use crate::utilities;
use crate::utilities::encoding::{Decode, Encode};
use crate::utilities::id::{generate_ephemera_id, EphemeraId};
use crate::utilities::time::ephemera_now;

pub type BlockHash = [u8; 32];

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct BlockHeader {
    pub(crate) id: EphemeraId,
    pub(crate) timestamp: u64,
    pub(crate) creator: PeerId,
    pub(crate) height: u64,
    pub(crate) hash: BlockHash,
}

impl BlockHeader {
    pub(crate) fn new(creator: PeerId, height: u64, hash: BlockHash) -> Self {
        Self {
            id: generate_ephemera_id(),
            timestamp: ephemera_now(),
            creator,
            height,
            hash,
        }
    }
}

impl Display for BlockHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let id = &self.id;
        let time = self.timestamp;
        let creator = &self.creator;
        let height = self.height;
        write!(
            f,
            "id: {id}, timestamp: {time}, creator: {creator}, height: {height}",
        )
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct Block {
    pub(crate) header: BlockHeader,
    pub(crate) messages: Vec<EphemeraMessage>,
}

impl Block {
    pub(crate) fn new(raw_block: RawBlock, hash: BlockHash) -> Self {
        let header = BlockHeader::new(raw_block.header.creator, raw_block.header.height, hash);
        Self {
            header,
            messages: raw_block.signed_messages,
        }
    }

    pub(crate) fn get_block_id(&self) -> EphemeraId {
        self.header.id.clone()
    }

    pub(crate) fn new_genesis_block(peer_id: PeerId) -> Self {
        Self {
            header: BlockHeader {
                id: generate_ephemera_id(),
                timestamp: ephemera_now(),
                creator: peer_id,
                height: 0,
                hash: [0; 32],
            },
            messages: Vec::new(),
        }
    }
}

impl Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let header = &self.header;
        write!(f, "{header}, nr of messages: {}", self.messages.len())
    }
}

impl From<Block> for RawBlock {
    fn from(block: Block) -> Self {
        Self {
            header: block.header.into(),
            signed_messages: block.messages,
        }
    }
}

impl From<&Block> for &RawBlock {
    fn from(block: &Block) -> Self {
        let raw_block: RawBlock = block.clone().into();
        Box::leak(Box::new(raw_block))
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct RawBlockHeader {
    pub(crate) creator: PeerId,
    pub(crate) height: u64,
}

impl RawBlockHeader {
    pub(crate) fn new(creator: PeerId, height: u64) -> Self {
        Self { creator, height }
    }
}

impl Display for RawBlockHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let creator = &self.creator;
        let height = self.height;
        write!(f, "creator: {creator}, height: {height}",)
    }
}

impl From<BlockHeader> for RawBlockHeader {
    fn from(block_header: BlockHeader) -> Self {
        Self {
            creator: block_header.creator,
            height: block_header.height,
        }
    }
}

/// Raw block represents all the data what will be signed
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct RawBlock {
    pub(crate) header: RawBlockHeader,
    pub(crate) signed_messages: Vec<EphemeraMessage>,
}

impl RawBlock {
    pub(crate) fn new(header: RawBlockHeader, signed_messages: Vec<EphemeraMessage>) -> Self {
        Self {
            header,
            signed_messages,
        }
    }
}

impl Encode for RawBlock {
    fn encode(&self) -> anyhow::Result<Vec<u8>> {
        utilities::encoding::encode(self)
    }
}

impl Decode for RawBlock {
    fn decode(bytes: &[u8]) -> anyhow::Result<Self> {
        utilities::encoding::decode(bytes)
    }
}

//TODO: integrate with blake2_256
impl Hash for BlockHeader {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.creator.hash(state);
        self.height.hash(state);
    }
}

//TODO: integrate with blake2_256
impl Hash for Block {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.header.hash(state);
        self.messages.hash(state);
    }
}
