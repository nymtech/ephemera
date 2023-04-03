//! # Api Types
//!
//! This module contains all the types that are used in the API.
//! Basically they are public versions of the same types used internally.
//! But it seems like a good idea to keep external and internal types separate.

use serde::{Deserialize, Serialize};
use std::fmt::Display;
use utoipa::ToSchema;

use crate::{
    api::ApiError,
    block::{
        types::block::BlockHeader,
        types::{block::Block, message::EphemeraMessage},
    },
    codec::{Decode, Encode, EphemeraEncoder},
    crypto::{Keypair, PublicKey},
    network::peer::PeerId,
    utilities::{
        crypto::{Certificate, Signature},
        encoding::{Decoder, Encoder, EphemeraDecoder},
        time::EphemeraTime,
    },
};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct ApiEphemeraMessage {
    /// The timestamp of the message.
    pub timestamp: u64,
    /// The label of the message. It can be used to identify the type of a message for example.
    pub label: String,
    /// The data of the message. It is application specific.
    pub data: Vec<u8>,
    /// The signature of the message. It implements Default trait instead of using Option.
    pub signature: ApiCertificate,
}

impl ApiEphemeraMessage {
    pub fn new(raw_message: RawApiEphemeraMessage, certificate: ApiCertificate) -> Self {
        Self {
            timestamp: raw_message.timestamp,
            label: raw_message.label,
            data: raw_message.data,
            signature: certificate,
        }
    }
}

impl Display for ApiEphemeraMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ApiEphemeraMessage(timestamp: {}, label: {})",
            self.timestamp, self.label,
        )
    }
}

impl From<ApiEphemeraMessage> for RawApiEphemeraMessage {
    fn from(message: ApiEphemeraMessage) -> Self {
        RawApiEphemeraMessage {
            timestamp: message.timestamp,
            label: message.label,
            data: message.data,
        }
    }
}

impl From<ApiEphemeraMessage> for EphemeraMessage {
    fn from(message: ApiEphemeraMessage) -> Self {
        Self {
            timestamp: message.timestamp,
            label: message.label,
            data: message.data,
            certificate: message.signature.into(),
        }
    }
}

impl From<EphemeraMessage> for ApiEphemeraMessage {
    fn from(ephemera_message: EphemeraMessage) -> Self {
        Self {
            timestamp: ephemera_message.timestamp,
            label: ephemera_message.label,
            data: ephemera_message.data,
            signature: ApiCertificate {
                signature: ephemera_message.certificate.signature,
                public_key: ephemera_message.certificate.public_key,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct ApiCertificate {
    pub signature: Signature,
    pub public_key: PublicKey,
}

impl ApiCertificate {
    pub fn prepare<D: Encode>(key_pair: &Keypair, data: &D) -> anyhow::Result<Self> {
        Certificate::prepare(key_pair, data).map(|c| c.into())
    }

    pub fn verify<D: Encode>(&self, data: &D) -> anyhow::Result<bool> {
        let certificate: Certificate = (self.clone()).into();
        Certificate::verify(&certificate, data)
    }
}

impl From<Certificate> for ApiCertificate {
    fn from(signature: Certificate) -> Self {
        Self {
            signature: signature.signature,
            public_key: signature.public_key,
        }
    }
}

impl From<ApiCertificate> for Certificate {
    fn from(value: ApiCertificate) -> Self {
        Certificate {
            signature: value.signature,
            public_key: value.public_key,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct RawApiEphemeraMessage {
    pub timestamp: u64,
    pub label: String,
    pub data: Vec<u8>,
}

impl RawApiEphemeraMessage {
    pub fn new(label: String, data: Vec<u8>) -> Self {
        Self {
            timestamp: EphemeraTime::now(),
            label,
            data,
        }
    }
}

impl Decode for RawApiEphemeraMessage {
    type Output = Self;

    fn decode(bytes: &[u8]) -> anyhow::Result<Self::Output> {
        Decoder::decode(bytes)
    }
}

impl Encode for RawApiEphemeraMessage {
    fn encode(&self) -> anyhow::Result<Vec<u8>> {
        Encoder::encode(self)
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ApiBlockHeader {
    pub timestamp: u64,
    pub creator: PeerId,
    pub height: u64,
    pub hash: String,
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

    pub fn hash(&self) -> String {
        self.header.hash.clone()
    }

    pub fn verify(&self, certificate: &ApiCertificate) -> Result<bool, ApiError> {
        let block: Block = self.clone().try_into()?;
        let valid = block.verify(&(certificate.clone()).into())?;
        Ok(valid)
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
                timestamp: block.header.timestamp,
                creator: block.header.creator,
                height: block.header.height,
                hash: block.header.hash.to_string(),
            },
            messages: block
                .messages
                .into_iter()
                .map(|signed_message| signed_message.into())
                .collect(),
        }
    }
}

impl TryFrom<ApiBlock> for Block {
    type Error = ApiError;

    fn try_from(api_block: ApiBlock) -> Result<Self, ApiError> {
        let messages: Vec<EphemeraMessage> = api_block
            .messages
            .into_iter()
            .map(|message| message.into())
            .collect::<Vec<EphemeraMessage>>();
        Ok(Self {
            header: BlockHeader {
                timestamp: api_block.header.timestamp,
                creator: api_block.header.creator,
                height: api_block.header.height,
                hash: api_block.header.hash.parse()?,
            },
            messages,
        })
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
