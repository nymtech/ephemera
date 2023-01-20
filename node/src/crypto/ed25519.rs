use crate::crypto::{KeyPair, KeyPairError};
///! Uses the `ed25519-zebra` crate to implement signing and signature verification.
use ed25519_zebra::{Signature, SigningKey, VerificationKey};
use rand_chacha::rand_core::{RngCore, SeedableRng};

#[derive(Debug)]
pub struct Ed25519KeyPair {
    pub signing_key: SigningKey,
    pub verification_key: VerificationKey,
}

impl KeyPair for Ed25519KeyPair {
    type Signature = String;

    fn verify<M: AsRef<[u8]>>(&self, message: M, sig_data: &Self::Signature) -> Result<(), KeyPairError> {
        let sig_bytes = array_bytes::hex2bytes(sig_data).map_err(|_| KeyPairError::Signature)?;
        let signature = Signature::from(<[u8; 64]>::try_from(sig_bytes.as_slice()).unwrap());
        self.verification_key
            .verify(&signature, message.as_ref())
            .map_err(|_| KeyPairError::Signature)
    }

    fn sign_hex<M: AsRef<[u8]>>(&self, message: M) -> Result<Self::Signature, KeyPairError> {
        let signature = self.signing_key.sign(message.as_ref());
        let sig_data: [u8; 64] = signature.into();
        Ok(array_bytes::bytes2hex("", sig_data))
    }

    fn generate() -> Result<Self, KeyPairError> {
        let mut rng = rand::rngs::StdRng::from_entropy();
        let mut seed = [0u8; 32];
        rng.fill_bytes(&mut seed);
        let signing_key = SigningKey::try_from(&seed[..]).map_err(|_| KeyPairError::SliceLength)?;
        let verification_key = VerificationKey::from(&signing_key);
        Ok(Ed25519KeyPair {
            signing_key,
            verification_key,
        })
    }
}
