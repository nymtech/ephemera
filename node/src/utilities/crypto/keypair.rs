use crate::utilities::crypto::KeyPairError;

pub trait PublicKey {
    fn to_raw_vec(&self) -> Vec<u8>;
    fn from_raw_vec(raw: Vec<u8>) -> Result<Self, KeyPairError>
    where
        Self: Sized;
    fn verify<M: AsRef<[u8]>>(&self, msg: &M, signature: &[u8]) -> bool;
}

pub trait Keypair {
    type Signature;
    type PublicKey;

    fn generate_pair(seed: Option<Vec<u8>>) -> Self;

    fn sign<M: AsRef<[u8]>>(&self, msg: &M) -> Result<Self::Signature, KeyPairError>;

    fn verify<M: AsRef<[u8]>>(&self, msg: &M, signature: &Self::Signature) -> bool;

    fn to_raw_vec(&self) -> Vec<u8>;

    fn from_raw_vec(raw: Vec<u8>) -> Result<Self, KeyPairError>
    where
        Self: Sized;

    fn public_key(&self) -> Self::PublicKey;
}
