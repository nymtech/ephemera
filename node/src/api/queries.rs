use crate::database::query::DbQuery;

use crate::broadcast_protocol::signing::signer::{Signature, SignedConsensusMessage};
use crate::config::configuration::DbConfig;
use thiserror::Error;
use serde_derive::{Deserialize, Serialize};

#[derive(Error, Debug)]
pub enum MessagesApiError {
    #[error("Database error: '{}'", .0)]
    DbError(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EphemeraMessage {
    pub request_id: String,
    pub message: Vec<u8>,
    pub signatures: Vec<Signature>,
}

pub struct MessagesApi {
    db_query: DbQuery,
}

impl MessagesApi {
    pub fn new(db_config: DbConfig) -> Self {
        Self {
            db_query: DbQuery::new(db_config),
        }
    }

    pub fn get_message_by_request_id(&self, request_id: String) -> Result<Option<EphemeraMessage>, MessagesApiError> {
        let result = self.db_query.get_message_by_request_id(request_id);
        Self::match_result(result)
    }

    pub fn get_message_by_custom_message_id(&self, custom_message_id: String) -> Result<Option<EphemeraMessage>, MessagesApiError> {
        let result = self.db_query.get_message_by_custom_message_id(custom_message_id);
        Self::match_result(result)
    }

    fn match_result(result: anyhow::Result<Option<SignedConsensusMessage>>) -> Result<Option<EphemeraMessage>, MessagesApiError> {
        match result {
            Ok(Some(message)) => Ok(Some(EphemeraMessage {
                request_id: message.request_id,
                message: message.message,
                signatures: message.signatures.values().cloned().collect(),
            })),
            Ok(None) => Ok(None),
            Err(err) => Err(MessagesApiError::DbError(err.to_string())),
        }
    }
}
