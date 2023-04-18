use std::fmt::{Debug, Display};

use serde::{Deserialize, Serialize};

use crate::{
    block::types::message::EphemeraMessage,
    codec::{Decode, EphemeraEncoder},
    crypto::Keypair,
    peer::PeerId,
    utilities::encoding::{Decoder, EphemeraDecoder},
    utilities::hash::{HashType, Hasher},
    utilities::{
        crypto::Certificate,
        encoding::{Encode, Encoder},
        hash::{EphemeraHash, EphemeraHasher},
        time::EphemeraTime,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct BlockHeader {
    pub(crate) timestamp: u64,
    pub(crate) creator: PeerId,
    pub(crate) height: u64,
    pub(crate) hash: HashType,
}

impl BlockHeader {
    pub(crate) fn new(raw_header: RawBlockHeader, hash: HashType) -> Self {
        Self {
            timestamp: raw_header.timestamp,
            creator: raw_header.creator,
            height: raw_header.height,
            hash,
        }
    }
}

impl Display for BlockHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hash = &self.hash;
        let time = self.timestamp;
        let creator = &self.creator;
        let height = self.height;
        write!(
            f,
            "hash: {hash}, timestamp: {time}, creator: {creator}, height: {height}",
        )
    }
}

impl Encode for BlockHeader {
    fn encode(&self) -> anyhow::Result<Vec<u8>> {
        Encoder::encode(&self)
    }
}

impl Decode for BlockHeader {
    type Output = Self;

    fn decode(bytes: &[u8]) -> anyhow::Result<Self::Output> {
        Decoder::decode(bytes)
    }
}

impl EphemeraHash for BlockHeader {
    fn hash<H: EphemeraHasher>(&self, state: &mut H) -> anyhow::Result<()> {
        let bytes = Encoder::encode(&self)?;
        state.update(&bytes);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct RawBlockHeader {
    pub(crate) timestamp: u64,
    pub(crate) creator: PeerId,
    pub(crate) height: u64,
}

impl RawBlockHeader {
    pub(crate) fn new(creator: PeerId, height: u64) -> Self {
        Self {
            timestamp: EphemeraTime::now(),
            creator,
            height,
        }
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
            timestamp: block_header.timestamp,
            creator: block_header.creator,
            height: block_header.height,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct Block {
    pub(crate) header: BlockHeader,
    pub(crate) messages: Vec<EphemeraMessage>,
}

impl Block {
    pub(crate) fn new(raw_block: RawBlock, block_hash: HashType) -> Self {
        let header = BlockHeader::new(raw_block.header.clone(), block_hash);
        Self {
            header,
            messages: raw_block.messages,
        }
    }

    pub(crate) fn get_hash(&self) -> HashType {
        self.header.hash
    }

    pub(crate) fn get_height(&self) -> u64 {
        self.header.height
    }

    pub(crate) fn new_genesis_block(creator: PeerId) -> Self {
        Self {
            header: BlockHeader {
                timestamp: EphemeraTime::now(),
                creator,
                height: 0,
                hash: HashType::new([0; 32]),
            },
            messages: Vec::new(),
        }
    }

    pub(crate) fn sign(&self, keypair: &Keypair) -> anyhow::Result<Certificate> {
        let raw_block: RawBlock = self.clone().into();
        let certificate = Certificate::prepare(keypair, &raw_block)?;
        Ok(certificate)
    }

    pub(crate) fn verify(&self, certificate: &Certificate) -> anyhow::Result<bool> {
        let raw_block: RawBlock = self.clone().into();
        certificate.verify(&raw_block)
    }

    pub(crate) fn hash_with_default_hasher(&self) -> anyhow::Result<HashType> {
        let raw_block: RawBlock = self.clone().into();
        raw_block.hash_with_default_hasher()
    }
}

impl Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let header = &self.header;
        write!(f, "{header}, nr of messages: {}", self.messages.len())
    }
}

impl Encode for Block {
    fn encode(&self) -> anyhow::Result<Vec<u8>> {
        Encoder::encode(&self)
    }
}

impl Decode for Block {
    type Output = Block;

    fn decode(bytes: &[u8]) -> anyhow::Result<Self::Output> {
        Decoder::decode(bytes)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct RawBlock {
    pub(crate) header: RawBlockHeader,
    pub(crate) messages: Vec<EphemeraMessage>,
}

impl RawBlock {
    pub(crate) fn new(header: RawBlockHeader, messages: Vec<EphemeraMessage>) -> Self {
        Self { header, messages }
    }

    pub(crate) fn hash_with_default_hasher(&self) -> anyhow::Result<HashType> {
        let mut hasher = Hasher::default();
        self.hash(&mut hasher)?;
        let block_hash = hasher.finish().into();
        Ok(block_hash)
    }
}

impl From<Block> for RawBlock {
    fn from(block: Block) -> Self {
        Self {
            header: block.header.into(),
            messages: block.messages,
        }
    }
}

impl Encode for RawBlockHeader {
    fn encode(&self) -> anyhow::Result<Vec<u8>> {
        Encoder::encode(&self)
    }
}

impl Decode for RawBlockHeader {
    type Output = RawBlockHeader;

    fn decode(bytes: &[u8]) -> anyhow::Result<Self::Output> {
        Decoder::decode(bytes)
    }
}

impl Encode for RawBlock {
    fn encode(&self) -> anyhow::Result<Vec<u8>> {
        Encoder::encode(&self)
    }
}

impl Decode for RawBlock {
    type Output = RawBlock;

    fn decode(bytes: &[u8]) -> anyhow::Result<Self::Output> {
        Decoder::decode(bytes)
    }
}

impl EphemeraHash for RawBlockHeader {
    fn hash<H: EphemeraHasher>(&self, state: &mut H) -> anyhow::Result<()> {
        state.update(&self.encode()?);
        Ok(())
    }
}

impl EphemeraHash for RawBlock {
    fn hash<H: EphemeraHasher>(&self, state: &mut H) -> anyhow::Result<()> {
        self.header.hash(state)?;
        for message in &self.messages {
            message.hash(state)?;
        }
        Ok(())
    }
}
