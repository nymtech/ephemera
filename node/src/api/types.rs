use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::block::{Block, RawMessage, SignedMessage};
use crate::broadcast::PeerId;
use crate::utilities::crypto::Signature;
use crate::utilities::time::duration_now;
use crate::utilities::EphemeraId;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct ApiSignedMessage {
    pub request_id: String,
    pub timestamp: u128,
    pub label: String,
    pub data: String,
    pub signature: ApiSignature,
}

impl ApiSignedMessage {
    pub fn new(request_id: String, data: String, signature: ApiSignature, label: String) -> Self {
        Self {
            request_id,
            timestamp: duration_now().as_millis(),
            label,
            data,
            signature,
        }
    }
}

impl From<ApiSignedMessage> for ApiRawMessage {
    fn from(signed_message: ApiSignedMessage) -> Self {
        ApiRawMessage {
            request_id: signed_message.request_id,
            data: signed_message.data,
        }
    }
}

impl From<ApiRawMessage> for RawMessage {
    fn from(raw_message: ApiRawMessage) -> Self {
        RawMessage::new(raw_message.request_id, raw_message.data)
    }
}

impl From<ApiSignedMessage> for SignedMessage {
    fn from(signed_message: ApiSignedMessage) -> Self {
        SignedMessage::new(
            signed_message.request_id,
            signed_message.data,
            signed_message.signature.into(),
            signed_message.label,
        )
    }
}

impl From<SignedMessage> for ApiSignedMessage {
    fn from(signed_message: SignedMessage) -> Self {
        Self {
            request_id: signed_message.id,
            timestamp: signed_message.timestamp,
            label: signed_message.label,
            data: signed_message.data,
            signature: ApiSignature {
                signature: signed_message.signature.signature,
                public_key: signed_message.signature.public_key,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct ApiSignature {
    /// Signature in hex format
    pub signature: String,
    /// Public key in hex format
    pub public_key: String,
}

impl ApiSignature {
    pub fn new(signature: String, public_key: String) -> Self {
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
pub struct ApiRawMessage {
    pub request_id: String,
    pub data: String,
}

impl ApiRawMessage {
    pub fn new(request_id: String, data: String) -> Self {
        Self { request_id, data }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ApiBlockHeader {
    pub id: EphemeraId,
    pub timestamp: u128,
    pub creator: PeerId,
    pub height: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct ApiBlock {
    pub header: ApiBlockHeader,
    pub signed_messages: Vec<ApiSignedMessage>,
}

impl ApiBlock {
    pub fn as_raw_block(&self) -> ApiRawBlock {
        ApiRawBlock {
            header: self.header.clone(),
            signed_messages: self.signed_messages.to_vec(),
        }
    }
}

/// Raw block represents all the data what will be signed
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ApiRawBlock {
    pub(crate) header: ApiBlockHeader,
    pub(crate) signed_messages: Vec<ApiSignedMessage>,
}

impl ApiRawBlock {
    pub fn new(header: ApiBlockHeader, signed_messages: Vec<ApiSignedMessage>) -> Self {
        Self {
            header,
            signed_messages,
        }
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
            signed_messages: block
                .signed_messages
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
