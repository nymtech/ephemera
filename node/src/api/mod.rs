use crate::broadcast_protocol::signing::signer::Signature;

use crate::broadcast_protocol::EphemeraSigningRequest;

use serde_derive::{Deserialize, Serialize};
use utoipa::{ToSchema};
use thiserror::Error;

/// 'Send' module allows external systems submit messages to "blockchain". It is not possible to 'save' anything directly in blockchain but
/// only through this module.
pub mod send;

/// Query module allows to query immutable "blockchain"
pub mod queries;

#[derive(Error, Debug)]
pub enum MessagesApiError {
    #[error("Database error: '{}'", .0)]
    DbError(String),
}

#[derive(Serialize, Debug, Clone)]
pub struct SignedEphemeraMessageResponse {
    pub request_id: String,
    pub custom_message_id: String,
    pub message: Vec<u8>,
    pub signatures: Vec<Signature>,
}

#[derive(Deserialize, Debug, Clone, ToSchema)]
pub struct MessageSigningRequest {
    pub request_id: String,
    pub custom_message_id: String,
    pub message: Vec<u8>,
}
