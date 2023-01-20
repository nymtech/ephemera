use crate::broadcast_protocol::signing::signer::SignedConsensusMessage;
use rusqlite::{params, Connection, OptionalExtension};

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

    pub(crate) fn get_message(&self, request_id: String) -> Result<Option<SignedConsensusMessage>> {
        let mut stmt = self
            .connection
            .prepare_cached("SELECT request_id, message, signatures FROM messages WHERE request_id = ?1")?;
        let message = stmt.query_row(params![request_id], |row| {
            let request_id: String = row.get(0)?;
            let message: Vec<u8> = row.get(1)?;
            let signatures: Vec<u8> = row.get(2)?;
            let signatures = serde_json::from_slice(&signatures).map_err(|e| {
                log::error!("Error deserializing signatures: {}", e);
                rusqlite::Error::InvalidQuery {}
            })?;
            Ok(SignedConsensusMessage {
                request_id,
                message,
                signatures,
            })
        }).optional()?;
        Ok(message)
    }
}
