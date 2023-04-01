use crate::network::peer::{PeerId, ToPeerId};
use crate::utilities::crypto::keypair::KeyPairError;
use crate::utilities::crypto::{EphemeraPublicKey, Signature};
use crate::utilities::EphemeraKeypair;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

// Internally uses libp2p for now
pub struct Ed25519Keypair(pub(crate) libp2p::identity::Keypair);

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ed25519PublicKey(libp2p::identity::PublicKey);

impl Ed25519PublicKey {
    pub(crate) fn inner(&self) -> &libp2p::identity::PublicKey {
        &self.0
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        self.0.to_protobuf_encoding()
    }
}

impl Ed25519Keypair {
    pub(crate) fn inner(&self) -> &libp2p::identity::Keypair {
        &self.0
    }
}

impl Serialize for Ed25519PublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_base58())
    }
}

impl<'de> Deserialize<'de> for Ed25519PublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ed25519PublicKey::from_base58(&s).map_err(serde::de::Error::custom)
    }
}

impl Display for Ed25519PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_base58())
    }
}

/// A wrapper around the libp2p Keypair type.
/// libp2p internally supports different key types, we only use Ed25519.
impl EphemeraKeypair for Ed25519Keypair {
    type Signature = Signature;
    type PublicKey = Ed25519PublicKey;

    fn generate(_seed: Option<Vec<u8>>) -> Self {
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        Ed25519Keypair(keypair)
    }

    fn sign<M: AsRef<[u8]>>(&self, msg: &M) -> Result<Self::Signature, KeyPairError> {
        self.inner()
            .sign(msg.as_ref())
            .map_err(|_| KeyPairError::Signature)
            .map(Signature)
    }

    fn verify<M: AsRef<[u8]>>(&self, msg: &M, signature: &Self::Signature) -> bool {
        self.0.public().verify(msg.as_ref(), signature.as_ref())
    }

    fn to_raw_vec(&self) -> Vec<u8> {
        self.inner().to_protobuf_encoding().unwrap()
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

impl EphemeraPublicKey for Ed25519PublicKey {
    type Signature = Signature;

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

    fn verify<M: AsRef<[u8]>>(&self, msg: &M, signature: &Self::Signature) -> bool {
        self.0.verify(msg.as_ref(), signature.as_ref())
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
