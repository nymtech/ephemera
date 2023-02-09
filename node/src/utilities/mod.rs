use uuid::Uuid;

pub use crate::utilities::crypto::Signer;
pub use crate::utilities::crypto::signer::CryptoApi;
pub use crate::utilities::encoding::to_hex;

pub(crate) mod crypto;
pub(crate) mod encoding;
pub(crate) mod hash;
pub(crate) mod time;

pub(crate) type EphemeraId = String;

pub(crate) fn generate_ephemera_id() -> EphemeraId {
    Uuid::new_v4().to_string()
}
