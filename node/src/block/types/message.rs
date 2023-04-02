use serde::{Deserialize, Serialize};

use crate::codec::Decode;
use crate::utilities::encoding::{Decoder, EphemeraDecoder};
use crate::utilities::hash::{HashType, Hasher};
use crate::{
    codec::Encode,
    crypto::Keypair,
    utilities::crypto::Certificate,
    utilities::encoding::Encoder,
    utilities::hash::{EphemeraHash, EphemeraHasher},
    utilities::time::EphemeraTime,
    utilities::EphemeraEncoder,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub(crate) struct EphemeraMessage {
    pub(crate) timestamp: u64,
    ///Application specific logical identifier of the message
    pub(crate) label: String,
    ///Application specific data
    pub(crate) data: Vec<u8>,
    ///Signature of the raw message
    pub(crate) certificate: Certificate,
}

impl EphemeraMessage {
    pub(crate) fn new(raw_message: UnsignedEphemeraMessage, certificate: Certificate) -> Self {
        Self {
            timestamp: EphemeraTime::now(),
            label: raw_message.label,
            data: raw_message.data,
            certificate,
        }
    }

    pub(crate) fn signed(label: String, data: Vec<u8>, keypair: &Keypair) -> anyhow::Result<Self> {
        let raw_message = UnsignedEphemeraMessage::new(label, data);
        let certificate = Certificate::prepare(keypair, &raw_message)?;
        Ok(Self::new(raw_message, certificate))
    }

    pub(crate) fn hash_with_default_hasher(&self) -> anyhow::Result<HashType> {
        let mut hasher = Hasher::default();
        self.hash(&mut hasher)?;
        let hash = hasher.finish().into();
        Ok(hash)
    }
}

impl Encode for EphemeraMessage {
    fn encode(&self) -> anyhow::Result<Vec<u8>> {
        Encoder::encode(&self)
    }
}

impl Decode for EphemeraMessage {
    type Output = Self;

    fn decode(bytes: &[u8]) -> anyhow::Result<Self::Output> {
        Decoder::decode(bytes)
    }
}

impl EphemeraHash for EphemeraMessage {
    fn hash<H: EphemeraHasher>(&self, state: &mut H) -> anyhow::Result<()> {
        state.update(&self.encode()?);
        Ok(())
    }
}

/// Raw message represents all the data what will be signed.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct UnsignedEphemeraMessage {
    pub(crate) label: String,
    pub(crate) data: Vec<u8>,
}

impl UnsignedEphemeraMessage {
    pub(crate) fn new(label: String, data: Vec<u8>) -> Self {
        Self { label, data }
    }
}

impl From<EphemeraMessage> for UnsignedEphemeraMessage {
    fn from(message: EphemeraMessage) -> Self {
        Self {
            label: message.label,
            data: message.data,
        }
    }
}

impl Encode for UnsignedEphemeraMessage {
    fn encode(&self) -> anyhow::Result<Vec<u8>> {
        Encoder::encode(&self)
    }
}

#[cfg(test)]
mod test {
    use crate::crypto::{EphemeraKeypair, EphemeraPublicKey, Keypair};

    use super::*;

    #[test]
    fn test_sign_ok() {
        let message_signing_keypair = Keypair::generate(None);

        let message =
            EphemeraMessage::signed("test".to_string(), vec![1, 2, 3], &message_signing_keypair)
                .unwrap();
        let certificate = message.certificate.clone();
        let raw_message: UnsignedEphemeraMessage = message.into();
        let data = raw_message.encode().unwrap();

        assert!(certificate.public_key.verify(&data, &certificate.signature));
    }

    #[test]
    fn test_sign_fail() {
        let message_signing_keypair = Keypair::generate(None);

        let message =
            EphemeraMessage::signed("test".to_string(), vec![1, 2, 3], &message_signing_keypair)
                .unwrap();
        let certificate = message.certificate.clone();

        let modified_message = EphemeraMessage::signed(
            "test_test".to_string(),
            vec![1, 2, 3],
            &message_signing_keypair,
        )
        .unwrap();
        let raw_message: UnsignedEphemeraMessage = modified_message.into();
        let data = raw_message.encode().unwrap();

        assert!(!certificate.public_key.verify(&data, &certificate.signature));
    }
}
