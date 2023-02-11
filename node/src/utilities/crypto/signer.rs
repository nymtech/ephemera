use std::fmt::Debug;
use std::sync::Arc;

use serde::Serialize;

use crate::api::types::{ApiKeypair, ApiSignature};
use crate::block::RawMessage;
use crate::utilities::crypto::libp2p2_crypto::Libp2pKeypair;
use crate::utilities::crypto::{KeyPair, KeyPairError, Signature, Signer};
use crate::utilities::hash::hasher::Hasher;

pub struct Libp2pSigner {
    key_pair: Arc<Libp2pKeypair>,
}

impl Libp2pSigner {
    pub fn new(key_pair: Arc<Libp2pKeypair>) -> Self {
        Self { key_pair }
    }

    fn bytes<T: Serialize>(data: &T) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(&data)
    }

    fn hash<T: AsRef<[u8]>>(data: T) -> [u8; 32] {
        Hasher::keccak_256(data.as_ref())
    }

    fn hash_serde_keccak_256<T: Serialize>(data: &T) -> Result<[u8; 32], KeyPairError> {
        let bytes = Self::bytes(&data).map_err(|_| KeyPairError::Serialization)?;
        let hash = Self::hash(bytes);
        Ok(hash)
    }
}

impl Signer for Libp2pSigner {
    fn sign<T: Serialize>(&self, data: &T) -> Result<Signature, KeyPairError> {
        let bytes = Self::bytes(&data).map_err(|_| KeyPairError::Serialization)?;
        let hash = Self::hash(bytes);

        let signature = (*self.key_pair).sign_hex(hash)?;
        let pub_key = self.key_pair.pub_key_to_hex()?;
        Ok(Signature::new(signature, pub_key))
    }

    fn verify<T: Serialize + Debug>(
        &self,
        data: &T,
        signature: &Signature,
    ) -> Result<bool, KeyPairError> {
        let hash = Libp2pSigner::hash_serde_keccak_256(data)?;
        Libp2pKeypair::verify_hex(hash, signature.public_key.clone(), &signature.signature)
    }
}

#[derive(Clone)]
pub struct CryptoApi;

impl CryptoApi {
    pub fn sign_message(
        request_id: String,
        data: String,
        private_key: String,
    ) -> Result<Signature, KeyPairError> {
        let keypair = Libp2pKeypair::from_private_key_hex(&private_key)?;
        let signer = get_default_signer(keypair)?;
        let raw_msg = RawMessage::new(request_id, data);
        let signature_hex = signer.sign(&raw_msg)?;
        Ok(signature_hex)
    }

    pub fn generate_keypair() -> Result<ApiKeypair, KeyPairError> {
        let keypair = Libp2pKeypair::generate()?;
        let private_key = keypair.private_key_to_hex()?;
        let public_key = keypair.pub_key_to_hex()?;
        Ok(ApiKeypair::new(public_key, private_key))
    }

    pub fn verify<T: Serialize + Debug>(
        data: &T,
        signature: &ApiSignature,
    ) -> Result<bool, KeyPairError> {
        let hash = Libp2pSigner::hash_serde_keccak_256(data)?;
        Libp2pKeypair::verify_hex(hash, signature.public_key.clone(), &signature.signature)
    }
}

fn get_default_signer(kp: Libp2pKeypair) -> Result<Libp2pSigner, KeyPairError> {
    Ok(Libp2pSigner::new(Arc::new(kp)))
}
