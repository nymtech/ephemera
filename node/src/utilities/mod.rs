use uuid::Uuid;

pub use crypto::Ed25519Keypair;
pub use crypto::Ed25519PublicKey;
pub use crypto::Keypair;
pub use crypto::PeerId;
pub use crypto::PublicKey;
pub use crypto::ToPeerId;
pub use encoding::encode;

pub use crate::utilities::encoding::from_base58;
pub use crate::utilities::encoding::from_hex;
pub use crate::utilities::encoding::to_hex;
pub use crate::utilities::encoding::{decode, Encode};

pub(crate) mod crypto;
pub(crate) mod encoding;
pub(crate) mod hash;
pub(crate) mod time;

pub(crate) type EphemeraId = String;

pub(crate) fn generate_ephemera_id() -> EphemeraId {
    Uuid::new_v4().to_string()
}
