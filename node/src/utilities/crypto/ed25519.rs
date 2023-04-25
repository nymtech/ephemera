use std::fmt::Display;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::crypto::EphemeraKeypair;
use crate::peer::{PeerId, ToPeerId};
use crate::utilities::crypto::keypair::KeyPairError;
use crate::utilities::crypto::{EphemeraPublicKey, Signature};

// Internally uses libp2p for now
pub struct Keypair(pub(crate) libp2p::identity::Keypair);

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PublicKey(libp2p::identity::PublicKey);

impl PublicKey {
    pub(crate) fn inner(&self) -> &libp2p::identity::PublicKey {
        &self.0
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        self.0.to_protobuf_encoding()
    }
}

impl Keypair {
    pub(crate) fn inner(&self) -> &libp2p::identity::Keypair {
        &self.0
    }
}

impl Display for Keypair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Public key: {}, Secret key: .........",
            self.public_key().to_base58()
        )
    }
}

impl Serialize for PublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_base58())
    }
}

impl<'de> Deserialize<'de> for PublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        PublicKey::from_base58(&s).map_err(serde::de::Error::custom)
    }
}

impl Display for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_base58())
    }
}

impl FromStr for PublicKey {
    type Err = KeyPairError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        PublicKey::from_base58(s)
    }
}

/// A wrapper around the libp2p Keypair type.
/// libp2p internally supports different key types, we only use Ed25519.
impl EphemeraKeypair for Keypair {
    type Signature = Signature;
    type PublicKey = PublicKey;

    fn generate(_seed: Option<Vec<u8>>) -> Self {
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        Keypair(keypair)
    }

    fn sign<M: AsRef<[u8]>>(&self, msg: &M) -> Result<Self::Signature, KeyPairError> {
        self.inner()
            .sign(msg.as_ref())
            .map_err(|err| KeyPairError::Signing(err.to_string()))
            .map(Signature)
    }

    fn verify<M: AsRef<[u8]>>(&self, msg: &M, signature: &Self::Signature) -> bool {
        self.0.public().verify(msg.as_ref(), signature.as_ref())
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.inner().to_protobuf_encoding().unwrap()
    }

    fn from_bytes(raw: Vec<u8>) -> Result<Self, KeyPairError>
    where
        Self: Sized,
    {
        let keypair = libp2p::identity::Keypair::from_protobuf_encoding(&raw)
            .map_err(|err| KeyPairError::Decoding(err.to_string()))?;
        Ok(Keypair(keypair))
    }

    fn public_key(&self) -> Self::PublicKey {
        PublicKey(self.0.public())
    }
}

impl EphemeraPublicKey for PublicKey {
    type Signature = Signature;

    fn to_bytes(&self) -> Vec<u8> {
        self.0.to_protobuf_encoding()
    }

    fn from_bytes(raw: Vec<u8>) -> Result<Self, KeyPairError>
    where
        Self: Sized,
    {
        let public_key = libp2p::identity::PublicKey::from_protobuf_encoding(&raw)
            .map_err(|err| KeyPairError::Decoding(err.to_string()))?;
        Ok(PublicKey(public_key))
    }

    fn verify<M: AsRef<[u8]>>(&self, msg: &M, signature: &Self::Signature) -> bool {
        self.0.verify(msg.as_ref(), signature.as_ref())
    }
}

impl ToPeerId for Keypair {
    fn peer_id(&self) -> PeerId {
        PeerId(self.0.public().to_peer_id())
    }
}

impl ToPeerId for PublicKey {
    fn peer_id(&self) -> PeerId {
        PeerId(self.0.to_peer_id())
    }
}
