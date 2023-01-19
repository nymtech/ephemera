use anyhow::Result;
use thiserror::Error;

use crate::broadcast_protocol::signing::signer::SignedConsensusMessage;
use chrono::Utc;
use rusqlite::{params, Connection};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

mod migrations {
    use refinery::embed_migrations;
    embed_migrations!("migrations");
}

#[derive(Error, Debug)]
pub enum DbBackendError {
    #[error("'{}'", .0)]
    Other(String),
}

pub enum DbBackendCmd {
    STORE(SignedConsensusMessage),
}

#[derive(Clone)]
pub struct DbBackendHandle {
    cmd_tx: mpsc::Sender<DbBackendCmd>,
}

impl DbBackendHandle {
    pub fn new(cmd_tx: mpsc::Sender<DbBackendCmd>) -> DbBackendHandle {
        DbBackendHandle { cmd_tx }
    }
    pub async fn store(&self, request: SignedConsensusMessage) -> Result<()> {
        self.cmd_tx
            .send(DbBackendCmd::STORE(request))
            .await
            .map_err(|e| DbBackendError::Other(format!("Error sending cmd to db backend: {}", e)).into())
    }
}

pub struct DbStore {
    connection: Connection,
    cmd_rcv: mpsc::Receiver<DbBackendCmd>,
}

impl DbStore {
    fn new(connection: Connection, cmd_rcv: mpsc::Receiver<DbBackendCmd>) -> DbStore {
        DbStore { connection, cmd_rcv }
    }

    pub async fn start(db_path: String) -> Result<(DbBackendHandle, JoinHandle<()>)> {
        log::info!("Starting db backend with path: {}", db_path);

        let mut connection = Connection::open(db_path)?;
        DbStore::run_migrations(&mut connection)?;

        let (cmd_tx, cmd_rx) = mpsc::channel(1000);
        let handle = DbBackendHandle::new(cmd_tx);

        let join_handle = tokio::spawn(async move {
            let mut backend = DbStore::new(connection, cmd_rx);
            while let Some(cmd) = backend.cmd_rcv.recv().await {
                match cmd {
                    DbBackendCmd::STORE(request) => {
                        if let Err(err) = backend.insert_signatures(request) {
                            log::error!("Error inserting message: {}", err);
                        }
                    }
                }
            }
        });
        Ok((handle, join_handle))
    }

    fn insert_signatures(&self, req: SignedConsensusMessage) -> Result<()> {
        let signatures = serde_json::to_vec(&req.signatures).map_err(|e| anyhow::anyhow!(e))?;
        let created_at = Utc::now().to_rfc3339();
        let mut statement = self.connection.prepare_cached(
            "INSERT INTO messages (request_id, message, signatures, created_at) VALUES (?1, ?2, ?3, ?4)",
        )?;

        log::debug!("Inserting message: {:?}", req);
        statement.execute(params![&req.request_id, &req.message, &signatures, &created_at])?;
        Ok(())
    }

    pub fn run_migrations(connection: &mut Connection) -> Result<()> {
        log::info!("Running database migrations");
        match migrations::migrations::runner().run(connection) {
            Ok(ok) => {
                log::info!("Database migrations completed:{:?} ", ok);
                Ok(())
            }
            Err(err) => {
                log::error!("Database migrations failed: {}", err);
                Err(anyhow::anyhow!(err))
            }
        }
    }
}
