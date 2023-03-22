//! This module contains all the types that are used in the API.
//! Basically they are public version of the same types used internally.
//! But it seems like a good idea to keep external and internal types separate.

use std::fmt::Display;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::block::types::block::Block;
use crate::block::types::message::{EphemeraMessage, EphemeraRawMessage};
use crate::network::PeerId;
use crate::utilities;
use crate::utilities::crypto::Signature;
use crate::utilities::encoding::Encode;
use crate::utilities::id::generate_ephemera_id;
use crate::utilities::id::EphemeraId;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct ApiEphemeraMessage {
    /// The id of the message. It has meaning only for the creator of the message.
    pub id: String,
    /// The timestamp of the message.
    pub timestamp: u64,
    /// The label of the message. It can be used to identify the type of a message for example.
    pub label: String,
    /// The data of the message. It is application specific.
    pub data: Vec<u8>,
    /// The signature of the message. It implements Default trait instead of using Option.
    pub signature: ApiSignature,
}

impl ApiEphemeraMessage {
    pub fn new(data: Vec<u8>, signature: ApiSignature, label: String) -> Self {
        Self {
            id: generate_ephemera_id(),
            timestamp: utilities::time::ephemera_now(),
            label,
            data,
            signature,
        }
    }
}

impl Display for ApiEphemeraMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ApiEphemeraMessage(id: {}, timestamp: {}, label: {})",
            self.id, self.timestamp, self.label,
        )
    }
}

impl From<ApiEphemeraMessage> for ApiEphemeraRawMessage {
    fn from(message: ApiEphemeraMessage) -> Self {
        ApiEphemeraRawMessage {
            id: message.id,
            data: message.data,
        }
    }
}

impl From<ApiEphemeraRawMessage> for EphemeraRawMessage {
    fn from(raw_message: ApiEphemeraRawMessage) -> Self {
        EphemeraRawMessage::new(raw_message.id, raw_message.data)
    }
}

impl From<ApiEphemeraMessage> for EphemeraMessage {
    fn from(ephemera_message: ApiEphemeraMessage) -> Self {
        let raw_message: ApiEphemeraRawMessage = ephemera_message.clone().into();
        EphemeraMessage::new(
            raw_message.into(),
            ephemera_message.signature.into(),
            ephemera_message.label,
        )
    }
}

impl From<EphemeraMessage> for ApiEphemeraMessage {
    fn from(ephemera_message: EphemeraMessage) -> Self {
        Self {
            id: ephemera_message.id,
            timestamp: ephemera_message.timestamp,
            label: ephemera_message.label,
            data: ephemera_message.data,
            signature: ApiSignature {
                signature: ephemera_message.signature.signature,
                public_key: ephemera_message.signature.public_key,
            },
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct ApiSignature {
    pub signature: Vec<u8>,
    pub public_key: Vec<u8>,
}

impl ApiSignature {
    pub fn new(signature: Vec<u8>, public_key: Vec<u8>) -> Self {
        Self {
            signature,
            public_key,
        }
    }
}

impl From<Signature> for ApiSignature {
    fn from(signature: Signature) -> Self {
        Self {
            signature: signature.signature,
            public_key: signature.public_key,
        }
    }
}

impl From<ApiSignature> for Signature {
    fn from(value: ApiSignature) -> Self {
        Signature {
            signature: value.signature,
            public_key: value.public_key,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct ApiEphemeraRawMessage {
    pub id: String,
    pub data: Vec<u8>,
}

impl ApiEphemeraRawMessage {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            id: generate_ephemera_id(),
            data,
        }
    }
}

impl Encode for ApiEphemeraRawMessage {
    fn encode(&self) -> anyhow::Result<Vec<u8>> {
        utilities::encoding::encode(self)
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ApiBlockHeader {
    pub id: EphemeraId,
    pub timestamp: u64,
    pub creator: PeerId,
    pub height: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct ApiBlock {
    pub header: ApiBlockHeader,
    pub messages: Vec<ApiEphemeraMessage>,
}

impl ApiBlock {
    pub fn as_raw_block(&self) -> ApiRawBlock {
        ApiRawBlock {
            header: self.header.clone(),
            messages: self.messages.to_vec(),
        }
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

/// Raw block represents all the data what will be signed
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ApiRawBlock {
    pub(crate) header: ApiBlockHeader,
    pub(crate) messages: Vec<ApiEphemeraMessage>,
}

impl ApiRawBlock {
    pub fn new(header: ApiBlockHeader, messages: Vec<ApiEphemeraMessage>) -> Self {
        Self { header, messages }
    }
}

impl From<&Block> for &ApiBlock {
    fn from(block: &Block) -> Self {
        let api_block: ApiBlock = block.clone().into();
        Box::leak(Box::new(api_block))
    }
}

impl From<Block> for ApiBlock {
    fn from(block: Block) -> Self {
        Self {
            header: ApiBlockHeader {
                id: block.header.id,
                timestamp: block.header.timestamp,
                creator: block.header.creator,
                height: block.header.height,
            },
            messages: block
                .messages
                .into_iter()
                .map(|signed_message| signed_message.into())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct ApiKeypair {
    pub public_key: String,
    pub private_key: String,
}

impl ApiKeypair {
    pub fn new(public_key: String, private_key: String) -> Self {
        Self {
            public_key,
            private_key,
        }
    }
}
