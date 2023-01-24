use crate::broadcast_protocol::signing::signer::SignedConsensusMessage;
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::config::configuration::DbConfig;
use anyhow::Result;

pub(crate) struct DbQuery {
    pub(crate) connection: Connection,
}

impl DbQuery {
    pub(crate) fn new(db_conf: DbConfig) -> Self {
        match Connection::open(db_conf.db_path) {
            Ok(connection) => Self { connection },
            Err(err) => {
                panic!("Error opening database: {}", err);
            }
        }
    }

    pub(crate) fn get_message_by_request_id(
        &self,
        request_id: String,
    ) -> Result<Option<SignedConsensusMessage>> {
        let mut stmt = self.connection.prepare_cached(
            "SELECT request_id, custom_message_id, message, signatures FROM messages WHERE request_id = ?1",
        )?;
        let message = stmt.query_row(params![request_id], Self::map_row()).optional()?;
        Ok(message)
    }

    pub(crate) fn get_message_by_custom_message_id(
        &self,
        custom_message_id: String,
    ) -> Result<Option<SignedConsensusMessage>> {
        let mut stmt = self
            .connection
            .prepare_cached("SELECT request_id, custom_message_id, message, signatures FROM messages WHERE custom_message_id = ?1")?;
        let message = stmt
            .query_row(params![custom_message_id], Self::map_row())
            .optional()?;
        Ok(message)
    }

    fn map_row() -> impl FnOnce(&Row) -> core::result::Result<SignedConsensusMessage, rusqlite::Error> {
        |row| {
            let request_id: String = row.get(0)?;
            let custom_message_id: String = row.get(1)?;
            let message: Vec<u8> = row.get(2)?;
            let signatures: Vec<u8> = row.get(3)?;
            let signatures = serde_json::from_slice(&signatures).map_err(|e| {
                log::error!("Error deserializing signatures: {}", e);
                rusqlite::Error::InvalidQuery {}
            })?;
            Ok(SignedConsensusMessage {
                request_id,
                custom_message_id,
                message,
                signatures,
            })
        }
    }
}
