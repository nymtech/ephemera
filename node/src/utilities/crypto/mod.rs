use std::fmt::Debug;
use std::hash::Hash;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub(crate) mod ed25519;
pub mod key_manager;
pub(crate) mod keypair;
mod peer;

pub use ed25519::Ed25519Keypair;
pub use ed25519::Ed25519PublicKey;
pub use keypair::Keypair;
pub use keypair::PublicKey;
pub use peer::PeerId;
pub use peer::ToPeerId;

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
    #[error("Unable to deserialize keypair: '{}'", .0)]
    Deserialization(String),
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, Deserialize, Serialize)]
pub struct Signature {
    pub(crate) signature: Vec<u8>,
    pub(crate) public_key: Vec<u8>,
}

impl Signature {
    pub(crate) fn new(signature: Vec<u8>, public_key: Vec<u8>) -> Self {
        Self {
            signature,
            public_key,
        }
    }
}
