//! Crytographic module used for signing and verifying messages.

pub(crate) mod ed25519;
pub(crate) mod libp2p2_crypto;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum KeyPairError {
    #[error("Invalid key length")]
    SliceLength,
    #[error("Invalid key")]
    Signature,
    #[error("Invalid private key")]
    PrivateKey,
}

pub trait KeyPair: Sized {
    type Signature: AsRef<[u8]>;

    fn verify<M: AsRef<[u8]>>(&self, message: M, signature: &Self::Signature) -> Result<(), KeyPairError>;

    fn sign_hex<M: AsRef<[u8]>>(&self, message: M) -> Result<Self::Signature, KeyPairError>;

    fn generate() -> Result<Self, KeyPairError>;
}
