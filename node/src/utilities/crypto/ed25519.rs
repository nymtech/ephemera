use crate::network::{PeerId, ToPeerId};
use crate::utilities::crypto::{KeyPairError, PublicKey};
use crate::utilities::Keypair;

// Careful with DEBUG, DISPLAY!!!
// Internally uses libp2p for now
pub struct Ed25519Keypair(pub libp2p::identity::Keypair);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Ed25519PublicKey(pub libp2p::identity::PublicKey);

impl Ed25519Keypair {
    pub(crate) fn as_ref(&self) -> &libp2p::identity::Keypair {
        &self.0
    }
}

/// A wrapper around the libp2p Keypair type.
/// libp2p internally supports different key types, we only use Ed25519.
impl Keypair for Ed25519Keypair {
    type Signature = Vec<u8>;
    type PublicKey = Ed25519PublicKey;

    fn generate_pair(_seed: Option<Vec<u8>>) -> Self {
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        Ed25519Keypair(keypair)
    }

    fn sign<M: AsRef<[u8]>>(&self, msg: &M) -> Result<Self::Signature, KeyPairError> {
        self.as_ref()
            .sign(msg.as_ref())
            .map_err(|_| KeyPairError::Signature)
    }

    fn verify<M: AsRef<[u8]>>(&self, msg: &M, signature: &Self::Signature) -> bool {
        self.0.public().verify(msg.as_ref(), signature)
    }

    fn to_raw_vec(&self) -> Vec<u8> {
        self.as_ref().to_protobuf_encoding().unwrap()
    }

    fn from_raw_vec(raw: Vec<u8>) -> Result<Self, KeyPairError>
    where
        Self: Sized,
    {
        let keypair = libp2p::identity::Keypair::from_protobuf_encoding(&raw)
            .map_err(|e| KeyPairError::Deserialization(e.to_string()))?;
        Ok(Ed25519Keypair(keypair))
    }

    fn public_key(&self) -> Self::PublicKey {
        Ed25519PublicKey(self.0.public())
    }
}

impl PublicKey for Ed25519PublicKey {
    fn to_raw_vec(&self) -> Vec<u8> {
        self.0.to_protobuf_encoding()
    }

    fn from_raw_vec(raw: Vec<u8>) -> Result<Self, KeyPairError>
    where
        Self: Sized,
    {
        let public_key = libp2p::identity::PublicKey::from_protobuf_encoding(&raw)
            .map_err(|e| KeyPairError::Deserialization(e.to_string()))?;
        Ok(Ed25519PublicKey(public_key))
    }

    fn verify<M: AsRef<[u8]>>(&self, msg: &M, signature: &[u8]) -> bool {
        self.0.verify(msg.as_ref(), signature)
    }
}

impl ToPeerId for Ed25519Keypair {
    fn peer_id(&self) -> PeerId {
        PeerId(self.0.public().to_peer_id())
    }
}

impl ToPeerId for Ed25519PublicKey {
    fn peer_id(&self) -> PeerId {
        PeerId(self.0.to_peer_id())
    }
}
