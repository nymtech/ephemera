use blake2::{Blake2b, Digest};
use digest::consts::U32;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};

pub type Hasher = Blake2bHasher;

#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
pub struct HashType([u8; 32]);

impl HashType {
    pub fn new(hash: [u8; 32]) -> Self {
        Self(hash)
    }

    pub(crate) fn base58(&self) -> String {
        bs58::encode(self.0).into_string()
    }
}

impl Debug for HashType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.base58())
    }
}

impl Display for HashType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.base58())
    }
}

impl From<[u8; 32]> for HashType {
    fn from(hash: [u8; 32]) -> Self {
        Self(hash)
    }
}

/// A trait for hashing data.
pub(crate) trait EphemeraHasher: Default {
    /// Hashes the given data.
    fn digest(data: &[u8]) -> [u8; 32];

    /// Updates the hasher with the given data.
    fn update(&mut self, bytes: &[u8]);

    /// Finalizes the hasher and returns the hash.
    fn finish(&mut self) -> [u8; 32];
}

#[derive(Default)]
pub struct Blake2bHasher {
    hasher: Blake2b<U32>,
}

impl EphemeraHasher for Blake2bHasher {
    fn digest(data: &[u8]) -> [u8; 32] {
        let mut dest = [0; 32];
        type Blake2b256 = blake2::Blake2b<U32>;
        dest.copy_from_slice(Blake2b256::digest(data).as_slice());
        dest
    }

    fn update(&mut self, bytes: &[u8]) {
        self.hasher.update(bytes);
    }

    fn finish(&mut self) -> [u8; 32] {
        self.hasher.finalize_reset().into()
    }
}

pub(crate) trait EphemeraHash {
    fn hash<H: EphemeraHasher>(&self, state: &mut H) -> anyhow::Result<()>;
}
