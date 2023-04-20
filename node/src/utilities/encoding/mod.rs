pub(crate) mod varint_async;
pub(crate) mod varint_bytes;

use serde::Serialize;

pub type Encoder = SerdeEncoder;
pub type Decoder = SerdeDecoder;

/// Simple trait for encoding
pub trait EphemeraEncoder {
    /// Encodes a message into a vector of bytes
    fn encode<M: Serialize>(data: &M) -> anyhow::Result<Vec<u8>>;
}

/// Simple trait for decoding
pub trait EphemeraDecoder {
    /// Decodes a message from a vector of bytes
    fn decode<M: for<'de> serde::Deserialize<'de>>(bytes: &[u8]) -> anyhow::Result<M>;
}

pub struct SerdeEncoder;

impl EphemeraEncoder for SerdeEncoder {
    fn encode<M: Serialize>(data: &M) -> anyhow::Result<Vec<u8>> {
        let bytes = serde_json::to_vec(data).map_err(|e| anyhow::anyhow!(e))?;
        Ok(bytes)
    }
}

pub struct SerdeDecoder;

impl EphemeraDecoder for SerdeDecoder {
    fn decode<M: for<'de> serde::Deserialize<'de>>(bytes: &[u8]) -> anyhow::Result<M> {
        let decoded = serde_json::from_slice(bytes).map_err(|e| anyhow::anyhow!(e))?;
        Ok(decoded)
    }
}

/// Trait which types can implement to provide their own encoding
pub trait Encode {
    /// Encodes a message into a vector of bytes
    fn encode(&self) -> anyhow::Result<Vec<u8>>;
}

/// Trait which types can implement to provide their own decoding
pub trait Decode {
    type Output: for<'de> serde::Deserialize<'de>;

    /// Decodes a message from a vector of bytes
    fn decode(bytes: &[u8]) -> anyhow::Result<Self::Output>;
}
