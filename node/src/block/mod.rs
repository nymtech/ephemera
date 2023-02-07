use std::fmt::Display;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

use crate::broadcast::{BroadcastData, PeerId};
use crate::utilities::crypto::Signature;
use crate::utilities::id_generator::EphemeraId;
use crate::utilities::time::duration_now;

pub(crate) mod callback;
pub(crate) mod manager;
mod message_pool;
pub(crate) mod producer;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct BlockHeader {
    pub(crate) id: EphemeraId,
    pub(crate) timestamp: u128,
    pub(crate) creator: PeerId,
    pub(crate) height: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct Block {
    pub(crate) header: BlockHeader,
    pub(crate) signed_messages: Vec<SignedMessage>,
    pub(crate) signature: Signature,
}

impl Block {
    pub(crate) fn new(raw_block: RawBlock, signature: Signature) -> Self {
        Self {
            header: raw_block.header,
            signed_messages: raw_block.signed_messages,
            signature,
        }
    }
}

impl BroadcastData for Block {
    fn get_id(&self) -> EphemeraId {
        self.header.id.clone()
    }
}

impl Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let header = &self.header;
        write!(
            f,
            "{header}, nr of messages: {}",
            self.signed_messages.len()
        )
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

impl From<Block> for RawBlock {
    fn from(block: Block) -> Self {
        Self {
            header: block.header,
            signed_messages: block.signed_messages,
        }
    }
}

/// Raw block represents all the data what will be signed
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct RawBlock {
    pub(crate) header: BlockHeader,
    pub(crate) signed_messages: Vec<SignedMessage>,
}

impl RawBlock {
    pub(crate) fn new(header: BlockHeader, signed_messages: Vec<SignedMessage>) -> Self {
        Self {
            header,
            signed_messages,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct SignedMessage {
    pub(crate) id: EphemeraId,
    pub(crate) timestamp: u128,
    ///In hexadecimal format
    pub(crate) data: String,
    ///In hexadecimal format
    pub(crate) signature: Signature,
}

/// Raw message represents all the data what will be signed
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct RawMessage {
    pub(crate) id: String,
    pub(crate) data: String,
}

impl RawMessage {
    pub(crate) fn new(id: String, data: String) -> Self {
        Self { id, data }
    }
}

impl Hash for RawMessage {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.data.hash(state);
    }
}

impl From<SignedMessage> for RawMessage {
    fn from(signed_message: SignedMessage) -> Self {
        Self {
            id: signed_message.id,
            data: signed_message.data,
        }
    }
}

impl SignedMessage {
    pub(crate) fn new(id: String, data: String, signature: Signature) -> Self {
        Self {
            id,
            timestamp: duration_now().as_millis(),
            data,
            signature,
        }
    }
}

/// BlockHeader signature includes fields:
/// - id
/// - creator
/// - height
impl Hash for BlockHeader {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.creator.hash(state);
        self.height.hash(state);
    }
}

/// Block signature includes fields:
/// - header
/// - signed_messages
impl Hash for Block {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.header.hash(state);
        self.signed_messages.hash(state);
    }
}

/// SignedMessage signature includes fields:
/// - id
/// - data
impl Hash for SignedMessage {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.data.hash(state);
    }
}
