use serde::Serialize;

//TODO: JSON is fine for now
pub fn encode<M: Serialize>(message: M) -> anyhow::Result<Vec<u8>> {
    serde_json::to_vec(&message).map_err(|e| anyhow::anyhow!(e))
}

pub fn decode<M: for<'de> serde::Deserialize<'de>>(bytes: &[u8]) -> anyhow::Result<M> {
    serde_json::from_slice(bytes).map_err(|e| anyhow::anyhow!(e))
}

pub fn to_hex<T: AsRef<[u8]>>(data: T) -> String {
    array_bytes::bytes2hex("", data.as_ref())
}

pub fn from_hex<T: AsRef<[u8]>>(data: T) -> anyhow::Result<Vec<u8>> {
    array_bytes::hex2bytes(data.as_ref()).map_err(|_| anyhow::anyhow!("Invalid hex string"))
}

pub fn to_base58<T: AsRef<[u8]>>(data: T) -> String {
    bs58::encode(data.as_ref()).into_string()
}

pub fn from_base58<T: AsRef<[u8]>>(data: T) -> anyhow::Result<Vec<u8>> {
    bs58::decode(data.as_ref())
        .into_vec()
        .map_err(|_| anyhow::anyhow!("Invalid base58 string"))
}

pub trait Encode {
    fn encode(&self) -> anyhow::Result<Vec<u8>>;
}

pub trait Decode {
    fn decode(bytes: &[u8]) -> anyhow::Result<Self>
    where
        Self: Sized;
}
