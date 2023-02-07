use libp2p::identity::PublicKey;

use crate::utilities::crypto::{KeyPair, KeyPairError, KeypairHex};

// Forbid DEBUG, DISPLAY!!!
pub struct Libp2pKeypair(pub libp2p::identity::Keypair);

impl Libp2pKeypair {
    pub(crate) fn as_ref(&self) -> &libp2p::identity::Keypair {
        &self.0
    }
}

impl KeyPair for Libp2pKeypair {
    type Signature = String;
    type PublicKey = PublicKey;

    fn from_private_key_hex(hex: &str) -> Result<Self, KeyPairError> {
        let bytes = array_bytes::hex2bytes(hex).map_err(|err| {
            log::error!("Error decoding hex: {:?}", err);
            KeyPairError::PrivateKey("Hex decoding error".into())
        })?;
        let keypair = libp2p::identity::Keypair::from_protobuf_encoding(bytes.as_slice())
            .map_err(|err| KeyPairError::PrivateKey(err.to_string()))?;
        Ok(Self(keypair))
    }

    fn verify_hex<M: AsRef<[u8]>>(
        message: M,
        pub_key: String,
        signature: &Self::Signature,
    ) -> Result<bool, KeyPairError> {
        let public_key = Self::pub_key_from_hex(pub_key)?;
        let signature = array_bytes::hex2bytes(signature).map_err(|_| KeyPairError::Signature)?;
        let valid = public_key.verify(message.as_ref(), &signature);
        Ok(valid)
    }

    fn sign_hex<M: AsRef<[u8]>>(&self, message: M) -> Result<Self::Signature, KeyPairError> {
        let sig_data = self
            .as_ref()
            .sign(message.as_ref())
            .map_err(|_| KeyPairError::Signature)?;
        let hex = array_bytes::bytes2hex("", sig_data);
        Ok(hex)
    }

    fn pub_key_to_hex(&self) -> Result<String, KeyPairError> {
        let pub_key: Vec<u8> = self.as_ref().public().to_protobuf_encoding();
        Ok(array_bytes::bytes2hex("", pub_key))
    }

    fn private_key_to_hex(&self) -> Result<String, KeyPairError> {
        let priv_key: Vec<u8> = self.as_ref().to_protobuf_encoding().map_err(|err| {
            log::error!("Error encoding private key: {:?}", err);
            KeyPairError::PrivateKey("Encoding error".into())
        })?;
        Ok(array_bytes::bytes2hex("", priv_key))
    }

    fn pub_key_from_hex(pub_key: String) -> Result<PublicKey, KeyPairError> {
        let bytes = array_bytes::hex2bytes(pub_key).map_err(|_| KeyPairError::PublicKey)?;
        PublicKey::from_protobuf_encoding(&bytes).map_err(|_| KeyPairError::PublicKey)
    }

    fn format_hex(&self) -> Result<KeypairHex, KeyPairError> {
        Ok(KeypairHex::new(
            self.private_key_to_hex()?,
            self.pub_key_to_hex()?,
        ))
    }

    fn generate() -> Result<Self, KeyPairError> {
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        Ok(Libp2pKeypair(keypair))
    }
}
