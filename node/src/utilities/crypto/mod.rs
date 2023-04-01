use std::fmt::{Debug, Display};
use std::hash::Hash;

use serde::{Deserialize, Serialize};

pub use ed25519::{Ed25519Keypair, Ed25519PublicKey};
pub use keypair::{EphemeraKeypair, EphemeraPublicKey, KeyPairError};

use crate::codec::Encode;
use crate::utilities::encoding::Encoder;
use crate::utilities::EphemeraEncoder;

pub mod ed25519;
pub mod key_manager;
mod keypair;

pub type Keypair = Ed25519Keypair;
pub type PublicKey = Ed25519PublicKey;

#[derive(Clone, PartialEq, Hash, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct Signature(Vec<u8>);

impl Signature {
    pub fn new(signature: Vec<u8>) -> Self {
        Self(signature)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.0.clone()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn inner(&self) -> &[u8] {
        &self.0
    }

    pub fn to_base58(&self) -> String {
        bs58::encode(self.0.clone()).into_string()
    }
}

impl Debug for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_base58())
    }
}

impl Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_base58())
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct Certificate {
    pub(crate) signature: Signature,
    pub(crate) public_key: PublicKey,
}

impl Certificate {
    pub fn prepare<D: Encode>(key_pair: &Keypair, data: &D) -> anyhow::Result<Self> {
        let data_bytes = data.encode()?;
        let signature = key_pair.sign(&data_bytes)?;
        let public_key = key_pair.public_key();
        Ok(Self::new(signature, public_key))
    }
}

impl AsRef<[u8]> for Signature {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl From<Vec<u8>> for Signature {
    fn from(signature: Vec<u8>) -> Self {
        Self::new(signature)
    }
}

impl Certificate {
    pub(crate) fn new(signature: Signature, public_key: PublicKey) -> Self {
        Self {
            signature,
            public_key,
        }
    }
}

impl Encode for Certificate {
    fn encode(&self) -> anyhow::Result<Vec<u8>> {
        let mut result = Encoder::encode(&self.signature)?;
        result.extend_from_slice(&self.public_key.encode()?);
        Ok(result)
    }
}

impl Encode for PublicKey {
    fn encode(&self) -> anyhow::Result<Vec<u8>> {
        Ok(self.to_bytes())
    }
}

impl Display for Certificate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Signature {}, PublicKey {}",
            self.signature.to_base58(),
            self.public_key.to_base58()
        )
    }
}
