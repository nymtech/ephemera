//! # Api Types
//!
//! This module contains all the types that are used in the API.
//!
//! ## Types
//!
//! - ApiEphemeraMessage
//! - ApiBlock
//! - ApiCertificate
//! - Health
//! - ApiEphemeraConfig

use std::fmt::Display;

use array_bytes::{bytes2hex, hex2bytes};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    api::ApiError,
    block::types::{block::Block, block::BlockHeader, message::EphemeraMessage},
    codec::{Decode, Encode, EphemeraEncoder},
    crypto::{Keypair, PublicKey},
    network::peer::PeerId,
    utilities::{
        crypto::{Certificate, Signature},
        encoding::{Decoder, Encoder, EphemeraDecoder},
        time::EphemeraTime,
    },
};

/// # Ephemera message.
///
/// An ApiEphemeraMessage submitted to an Ephemera node will be gossiped to other nodes.
/// And should be eventually included in a Ephemera block.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct ApiEphemeraMessage {
    /// The timestamp of the message.
    pub timestamp: u64,
    /// The label of the message. It can be used to identify the type of a message for example.
    pub label: String,
    /// The data of the message. It is application specific.
    pub data: Vec<u8>,
    /// The signature of the message. It implements Default trait instead of using Option.
    pub certificate: ApiCertificate,
}

impl ApiEphemeraMessage {
    pub fn new(raw_message: RawApiEphemeraMessage, certificate: ApiCertificate) -> Self {
        Self {
            timestamp: raw_message.timestamp,
            label: raw_message.label,
            data: raw_message.data,
            certificate,
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
            certificate: message.certificate.into(),
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

    pub fn sign(&self, keypair: &Keypair) -> anyhow::Result<ApiEphemeraMessage> {
        let certificate = Certificate::prepare(keypair, &self)?;
        let message = ApiEphemeraMessage::new(self.clone(), certificate.into());
        Ok(message)
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

impl Encode for &RawApiEphemeraMessage {
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

impl Display for ApiBlockHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ApiBlockHeader(timestamp: {}, creator: {}, height: {}, hash: {})",
            self.timestamp, self.creator, self.height, self.hash,
        )
    }
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

impl Display for ApiBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ApiBlock(header: {}, message_count: {})",
            self.header,
            self.message_count()
        )
    }
}

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

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct Health {
    pub(crate) status: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct ApiEphemeraConfig {
    pub protocol_address: String,
    pub api_address: String,
    pub websocket_address: String,
    pub public_key: String,
    pub block_producer: bool,
    pub block_creation_interval_sec: u64,
}

impl From<EphemeraMessage> for ApiEphemeraMessage {
    fn from(ephemera_message: EphemeraMessage) -> Self {
        Self {
            timestamp: ephemera_message.timestamp,
            label: ephemera_message.label,
            data: ephemera_message.data,
            certificate: ApiCertificate {
                signature: ephemera_message.certificate.signature,
                public_key: ephemera_message.certificate.public_key,
            },
        }
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

//FIXME
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
pub struct ApiDhtStoreRequest {
    /// The key to store the value under in hex format.
    key: String,
    /// The value to store in hex format.
    value: String,
}

impl ApiDhtStoreRequest {
    pub fn new(key: &[u8], value: &[u8]) -> Self {
        let key = bytes2hex("0x", key);
        let value = bytes2hex("0x", value);
        Self { key, value }
    }

    pub fn key(&self) -> Vec<u8> {
        //We can unwrap here because the key is always valid.
        hex2bytes(&self.key).expect("Failed to decode key")
    }

    pub fn value(&self) -> Vec<u8> {
        //We can unwrap here because the value is always valid.
        hex2bytes(&self.value).expect("Failed to decode value")
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct ApiDhtQueryRequest {
    /// The key to query for in hex format.
    key: String,
}

impl ApiDhtQueryRequest {
    pub fn new(key: &[u8]) -> Self {
        let key = bytes2hex("0x", key);
        Self { key }
    }

    pub fn key_encoded(&self) -> String {
        self.key.clone()
    }

    pub fn key(&self) -> Vec<u8> {
        hex2bytes(&self.key).expect("Failed to decode key")
    }

    pub(crate) fn parse_key(key: &str) -> Vec<u8> {
        hex2bytes(key).expect("Failed to decode key")
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct ApiDhtQueryResponse {
    /// The key that was queried for in hex format.
    key: String,
    /// The value that was stored under the queried key in hex format.
    value: String,
}

impl ApiDhtQueryResponse {
    pub(crate) fn new(key: Vec<u8>, value: Vec<u8>) -> Self {
        let key = bytes2hex("0x", &key);
        let value = bytes2hex("0x", &value);
        Self { key, value }
    }

    pub fn key(&self) -> Vec<u8> {
        hex2bytes(&self.key).expect("Failed to decode key")
    }

    pub fn value(&self) -> Vec<u8> {
        hex2bytes(&self.value).expect("Failed to decode value")
    }
}

#[cfg(test)]
mod test {
    use crate::crypto::{EphemeraKeypair, EphemeraPublicKey, Keypair};

    use super::*;

    #[test]
    fn test_sign_ok() {
        let message_signing_keypair = Keypair::generate(None);

        let message = RawApiEphemeraMessage::new("test".to_string(), vec![1, 2, 3]);
        let signed_message = message
            .sign(&message_signing_keypair)
            .expect("Failed to sign message");

        let certificate = signed_message.certificate;

        assert!(certificate
            .public_key
            .verify(&message.encode().unwrap(), &certificate.signature));
    }

    #[test]
    fn test_sign_fail() {
        let message_signing_keypair = Keypair::generate(None);

        let message = RawApiEphemeraMessage::new("test1".to_string(), vec![1, 2, 3]);
        let signed_message = message
            .sign(&message_signing_keypair)
            .expect("Failed to sign message");

        let certificate = signed_message.certificate;

        let modified_message = RawApiEphemeraMessage::new("test2".to_string(), vec![1, 2, 3]);
        assert!(!certificate
            .public_key
            .verify(&modified_message.encode().unwrap(), &certificate.signature));
    }
}
