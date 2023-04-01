use thiserror::Error;

#[derive(Error, Debug)]
pub enum KeyPairError {
    #[error("Failed to serialize")]
    Serialization,
    #[error("Invalid hexadecimal")]
    InvalidEncoding,
    #[error("Invalid key length")]
    SliceLength,
    #[error("Invalid key")]
    Signature,
    #[error("Invalid private key: '{}'", .0)]
    PrivateKey(String),
    #[error("Invalid public key")]
    PublicKey,
    #[error("Unable to deserialize keypair: '{}'", .0)]
    Deserialization(String),
}

pub trait EphemeraPublicKey {
    type Signature: AsRef<[u8]>;

    fn to_raw_vec(&self) -> Vec<u8>;

    fn from_raw_vec(raw: Vec<u8>) -> Result<Self, KeyPairError>
    where
        Self: Sized;
    fn verify<M: AsRef<[u8]>>(&self, msg: &M, signature: &Self::Signature) -> bool;

    fn to_base58(&self) -> String {
        bs58::encode(self.to_raw_vec()).into_string()
    }

    fn from_base58(base58: &str) -> Result<Self, KeyPairError>
    where
        Self: Sized,
    {
        let raw = bs58::decode(base58)
            .into_vec()
            .map_err(|_| KeyPairError::InvalidEncoding)?;
        Self::from_raw_vec(raw)
    }
}

pub trait EphemeraKeypair {
    type Signature;
    type PublicKey;

    fn generate(seed: Option<Vec<u8>>) -> Self;

    fn sign<M: AsRef<[u8]>>(&self, msg: &M) -> Result<Self::Signature, KeyPairError>;

    fn verify<M: AsRef<[u8]>>(&self, msg: &M, signature: &Self::Signature) -> bool;

    fn to_raw_vec(&self) -> Vec<u8>;

    fn from_raw_vec(raw: Vec<u8>) -> Result<Self, KeyPairError>
    where
        Self: Sized;

    fn public_key(&self) -> Self::PublicKey;

    fn to_base58(&self) -> String {
        bs58::encode(self.to_raw_vec()).into_string()
    }

    fn from_base58(base58: &str) -> Result<Self, KeyPairError>
    where
        Self: Sized,
    {
        let raw = bs58::decode(base58)
            .into_vec()
            .map_err(|_| KeyPairError::InvalidEncoding)?;
        Self::from_raw_vec(raw)
    }
}
