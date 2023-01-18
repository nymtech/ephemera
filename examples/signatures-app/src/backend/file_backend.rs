use crate::broadcast_callback::SignaturesConsensusRequest;
use std::fs::File;
use std::io::Write;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use anyhow::Result;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FileBackendError {
    #[error("'{}'", .0)]
    Other(String),
}

pub enum FileBackendCmd {
    STORE(SignaturesConsensusRequest),
}

#[derive(Clone)]
pub struct FileBackendHandle {
    cmd_tx: mpsc::Sender<FileBackendCmd>,
}

impl FileBackendHandle {
    pub fn new(cmd_tx: mpsc::Sender<FileBackendCmd>) -> FileBackendHandle {
        FileBackendHandle { cmd_tx }
    }
    pub async fn store(&self, request: SignaturesConsensusRequest) -> Result<()> {
        self.cmd_tx
            .send(FileBackendCmd::STORE(request))
            .await
            .map_err(|e| {
                FileBackendError::Other(format!("Error sending cmd to file backend: {}", e)).into()
            })
    }
}

pub struct SignaturesFileBackend {
    pub cmd_rcv: mpsc::Receiver<FileBackendCmd>,
}

impl SignaturesFileBackend {
    fn new(cmd_rcv: mpsc::Receiver<FileBackendCmd>) -> SignaturesFileBackend {
        SignaturesFileBackend { cmd_rcv }
    }

    pub fn start(signatures_file: String) -> Result<(FileBackendHandle, JoinHandle<()>)> {
        log::info!("Starting file backend with path: {}", signatures_file);

        let (cmd_tx, cmd_rx) = mpsc::channel(1000);
        let handle = FileBackendHandle::new(cmd_tx);
        let mut backend = SignaturesFileBackend::new(cmd_rx);

        let mut file = Self::open_file(signatures_file)?;

        let join_handle = tokio::spawn(async move {
            log::debug!("File backend started");
            while let Some(cmd) = backend.cmd_rcv.recv().await {
                match cmd {
                    FileBackendCmd::STORE(req) => {
                        if let Err(err) = Self::store(&mut file, req) {
                            log::error!("Error storing signature: {}", err);
                        }
                    }
                }
            }
        });
        Ok((handle, join_handle))
    }

    fn open_file(signatures_file: String) -> Result<File> {
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(signatures_file)
        {
            Ok(file) => Ok(file),
            Err(e) => {
                return Err(FileBackendError::Other(format!("Error opening file: {}", e)).into());
            }
        }
    }

    fn store(file: &mut File, req: SignaturesConsensusRequest) -> Result<()> {
        file.write_all(b"payload: ")?;
        file.write_all(req.payload.as_slice())?;
        file.write_all(b"\n")?;
        for (_, signer) in req.signatures {
            file.write_all(b"signer: ")?;
            file.write_all(signer.id.as_bytes())?;
            file.write_all(b"\n")?;
            file.write_all(b"signature: ")?;
            file.write_all(signer.signature.as_bytes())?;
            file.write_all(b"\n")?;
        }
        file.write_all(b"\n\n")?;
        Ok(())
    }
}
