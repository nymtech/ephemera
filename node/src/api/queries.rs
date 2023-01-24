use crate::database::query::DbQuery;

use crate::api::{MessagesApiError, SignedEphemeraMessageResponse};
use crate::broadcast_protocol::signing::signer::{SignedConsensusMessage};
use crate::config::configuration::DbConfig;



pub struct MessagesQueryApi {
    db_query: DbQuery,
}

impl MessagesQueryApi {
    pub fn new(db_config: DbConfig) -> Self {
        Self {
            db_query: DbQuery::new(db_config),
        }
    }

    pub fn get_message_by_request_id(
        &self,
        request_id: String,
    ) -> Result<Option<SignedEphemeraMessageResponse>, MessagesApiError> {
        let result = self.db_query.get_message_by_request_id(request_id);
        Self::match_result(result)
    }

    pub fn get_message_by_custom_message_id(
        &self,
        custom_message_id: String,
    ) -> Result<Option<SignedEphemeraMessageResponse>, MessagesApiError> {
        let result = self.db_query.get_message_by_custom_message_id(custom_message_id);
        Self::match_result(result)
    }

    fn match_result(
        result: anyhow::Result<Option<SignedConsensusMessage>>,
    ) -> Result<Option<SignedEphemeraMessageResponse>, MessagesApiError> {
        match result {
            Ok(Some(message)) => Ok(Some(SignedEphemeraMessageResponse {
                request_id: message.request_id,
                custom_message_id: message.custom_message_id,
                message: message.message,
                signatures: message.signatures.values().cloned().collect(),
            })),
            Ok(None) => Ok(None),
            Err(err) => Err(MessagesApiError::DbError(err.to_string())),
        }
    }
}
