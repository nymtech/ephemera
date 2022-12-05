///! Copy from broadcast workspace

use ed25519_zebra::{Signature, SigningKey, VerificationKey};

#[derive(Debug)]
pub enum KeyPairError {
    InvalidSliceLength,
    InvalidSignature,
}

pub trait KeyPair: Sized {
    type Seed: Default + AsRef<[u8]> + AsMut<[u8]> + Clone;
    type Signature: AsRef<[u8]>;

    fn verify<M: AsRef<[u8]>>(
        &self,
        message: M,
        signature: &Self::Signature,
    ) -> Result<(), KeyPairError>;

    fn sign<M: AsRef<[u8]>>(&self, message: M) -> Result<Self::Signature, KeyPairError>;

    fn generate(seed: &Self::Seed) -> Result<Self, KeyPairError>;
}

#[derive(Debug)]
pub struct Ed25519KeyPair {
    pub signing_key: SigningKey,
    pub verification_key: VerificationKey,
}

impl KeyPair for Ed25519KeyPair {
    type Seed = [u8; 32];
    type Signature = String;

    fn verify<M: AsRef<[u8]>>(
        &self,
        message: M,
        sig_data: &Self::Signature,
    ) -> Result<(), KeyPairError> {
        let sig_bytes =
            array_bytes::hex2bytes(sig_data).map_err(|_| KeyPairError::InvalidSignature)?;
        let signature = Signature::from(<[u8; 64]>::try_from(sig_bytes.as_slice()).unwrap());
        self.verification_key
            .verify(&signature, message.as_ref())
            .map_err(|_| KeyPairError::InvalidSignature)
    }

    fn sign<M: AsRef<[u8]>>(&self, message: M) -> Result<Self::Signature, KeyPairError> {
        let signature = self.signing_key.sign(message.as_ref());
        let sig_data: [u8; 64] = signature.into();
        Ok(array_bytes::bytes2hex("", &sig_data))
    }

    fn generate(seed: &Self::Seed) -> Result<Self, KeyPairError> {
        let signing_key =
            SigningKey::try_from(&seed[..]).map_err(|_| KeyPairError::InvalidSliceLength)?;
        let verification_key = VerificationKey::from(&signing_key);
        Ok(Ed25519KeyPair {
            signing_key,
            verification_key,
        })
    }
}
