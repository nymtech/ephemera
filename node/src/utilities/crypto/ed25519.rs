//! Uses the `ed25519-zebra` crate to implement signing and signature verification.
use ed25519_zebra::{Signature, SigningKey, VerificationKey};
use rand_chacha::rand_core::{RngCore, SeedableRng};

use crate::utilities::crypto::{KeyPair, KeyPairError, KeypairHex};

#[derive(Debug)]
pub struct Ed25519KeyPair {
    pub signing_key: SigningKey,
    pub verification_key: VerificationKey,
}

impl KeyPair for Ed25519KeyPair {
    type Signature = String;
    type PublicKey = VerificationKey;

    fn from_private_key_hex(hex: &str) -> Result<Self, KeyPairError> {
        array_bytes::hex2bytes(hex)
            .map_err(|err| {
                log::error!("Error decoding hex: {:?}", err);
                KeyPairError::PrivateKey("Hex decoding error".into())
            })
            .and_then(|bytes| {
                SigningKey::try_from(&bytes[..]).map_err(|err| {
                    log::error!("Error decoding private key: {:?}", err);
                    KeyPairError::PrivateKey("Private key decoding error".into())
                })
            })
            .map(|signing_key| {
                let verification_key = VerificationKey::from(&signing_key);
                Self {
                    signing_key,
                    verification_key,
                }
            })
    }

    fn verify_hex<M: AsRef<[u8]>>(
        message: M,
        pub_key: String,
        sig_data: &Self::Signature,
    ) -> Result<bool, KeyPairError> {
        let public_key = Self::pub_key_from_hex(pub_key)?;
        let sig_bytes = array_bytes::hex2bytes(sig_data).map_err(|_| KeyPairError::Signature)?;
        let signature = Signature::from(<[u8; 64]>::try_from(sig_bytes.as_slice()).unwrap());
        let message_bytes = array_bytes::hex2bytes(message.as_ref())
            .map_err(|_| KeyPairError::InvalidHexadecimal)?;
        let valid = public_key.verify(&signature, &message_bytes).is_ok();
        Ok(valid)
    }

    fn sign_hex<M: AsRef<[u8]>>(&self, message: M) -> Result<Self::Signature, KeyPairError> {
        let signature = self.signing_key.sign(message.as_ref());
        let sig_data: [u8; 64] = signature.into();
        Ok(array_bytes::bytes2hex("", sig_data))
    }

    fn pub_key_to_hex(&self) -> Result<String, KeyPairError> {
        let pub_key: [u8; 32] = self.verification_key.into();
        Ok(array_bytes::bytes2hex("", pub_key))
    }

    fn private_key_to_hex(&self) -> Result<String, KeyPairError> {
        let priv_key = self.signing_key.as_ref();
        Ok(array_bytes::bytes2hex("", priv_key))
    }

    fn pub_key_from_hex(pub_key: String) -> Result<Self::PublicKey, KeyPairError> {
        let bytes = array_bytes::hex2bytes(pub_key).map_err(|_| KeyPairError::PublicKey)?;
        let verification_key =
            VerificationKey::try_from(&bytes[..]).map_err(|_| KeyPairError::PublicKey)?;
        Ok(verification_key)
    }

    fn format_hex(&self) -> Result<KeypairHex, KeyPairError> {
        Ok(KeypairHex::new(
            self.private_key_to_hex()?,
            self.pub_key_to_hex()?,
        ))
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
