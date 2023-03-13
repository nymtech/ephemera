use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

use crate::utilities;
use crate::utilities::crypto::Signature;
use crate::utilities::id::EphemeraId;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct EphemeraMessage {
    pub(crate) id: EphemeraId,
    ///Unix timestamp in seconds
    pub(crate) timestamp: u64,
    ///Application specific logical identifier of the message
    pub(crate) label: String,
    ///Application specific data
    pub(crate) data: Vec<u8>,
    ///Signature of the raw message
    pub(crate) signature: Signature,
}

impl EphemeraMessage {
    pub(crate) fn new(
        raw_message: EphemeraRawMessage,
        signature: Signature,
        label: String,
    ) -> Self {
        Self {
            id: raw_message.id,
            timestamp: utilities::time::ephemera_now(),
            label,
            data: raw_message.data,
            signature,
        }
    }
}

/// Raw message represents all the data what will be signed.
//TODO decide how exactly use EphemeraRawMessage internally/externally
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct EphemeraRawMessage {
    /// It's up to the application to decide if the messages with same data can be
    /// considered as duplicates. If so, the application should use the same id for them.
    pub(crate) id: EphemeraId,
    pub(crate) data: Vec<u8>,
}

impl EphemeraRawMessage {
    pub(crate) fn new(id: EphemeraId, data: Vec<u8>) -> Self {
        Self { id, data }
    }
}

impl Hash for EphemeraRawMessage {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.data.hash(state);
    }
}

impl From<EphemeraMessage> for EphemeraRawMessage {
    fn from(signed_message: EphemeraMessage) -> Self {
        Self {
            id: signed_message.id,
            data: signed_message.data,
        }
    }
}

//TODO: integrate with blake2_256
impl Hash for EphemeraMessage {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.data.hash(state);
    }
}
