use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::utilities::crypto::libp2p2_crypto::Libp2pKeypair;

pub(crate) mod ed25519;
pub(crate) mod libp2p2_crypto;
pub mod signer;

#[derive(Error, Debug)]
pub enum KeyPairError {
    #[error("Failed to serialize")]
    Serialization,
    #[error("Invalid hexadecimal")]
    InvalidHexadecimal,
    #[error("Invalid key length")]
    SliceLength,
    #[error("Invalid key")]
    Signature,
    #[error("Invalid private key: '{}'", .0)]
    PrivateKey(String),
    #[error("Invalid public key")]
    PublicKey,
}

pub(crate) trait KeyPair: Sized {
    type Signature: AsRef<[u8]>;
    type PublicKey;

    fn from_private_key_hex(hex: &str) -> Result<Self, KeyPairError>;

    fn verify_hex<M: AsRef<[u8]>>(
        message: M,
        pub_key: String,
        signature: &Self::Signature,
    ) -> Result<bool, KeyPairError>;

    fn sign_hex<M: AsRef<[u8]>>(&self, message: M) -> Result<Self::Signature, KeyPairError>;

    fn pub_key_to_hex(&self) -> Result<String, KeyPairError>;

    fn private_key_to_hex(&self) -> Result<String, KeyPairError>;

    fn pub_key_from_hex(pub_key: String) -> Result<Self::PublicKey, KeyPairError>;

    fn format_hex(&self) -> Result<KeypairHex, KeyPairError>;

    fn generate() -> Result<Self, KeyPairError>;
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct KeypairHex {
    pub private_key: String,
    pub public_key: String,
}

impl KeypairHex {
    pub fn new(private_key: String, public_key: String) -> Self {
        Self {
            private_key,
            public_key,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Signature {
    pub(crate) signature: String,
    pub(crate) public_key: String,
}

impl Signature {
    pub(crate) fn new(signature: String, public_key: String) -> Self {
        Self {
            signature,
            public_key,
        }
    }
}

pub trait Signer {
    fn sign<T: Serialize>(&mut self, data: &T) -> Result<Signature, KeyPairError>;

    fn verify<T: Serialize + Debug>(
        &self,
        data: &T,
        signature: &Signature,
    ) -> Result<bool, KeyPairError>;
}

pub type EphemeraKeypair = Libp2pKeypair;
