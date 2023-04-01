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

pub fn to_hex<T: AsRef<[u8]>>(data: T) -> String {
    array_bytes::bytes2hex("", data.as_ref())
}

pub fn to_base58<T: AsRef<[u8]>>(data: T) -> String {
    bs58::encode(data.as_ref()).into_string()
}

pub fn from_base58<T: AsRef<[u8]>>(data: T) -> anyhow::Result<Vec<u8>> {
    bs58::decode(data.as_ref())
        .into_vec()
        .map_err(|_| anyhow::anyhow!("Invalid base58 string"))
}
