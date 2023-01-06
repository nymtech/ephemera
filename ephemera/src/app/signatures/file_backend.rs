///! Writes collected signatures to a file
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::app::signatures::broadcast_callback::Signer;
use crate::settings::Settings;

pub struct SignaturesBackend {
    pub signatures_file: PathBuf,
}

impl SignaturesBackend {
    pub fn new(settings: Settings) -> SignaturesBackend {
        SignaturesBackend {
            signatures_file: Path::new(&settings.signatures_file).to_path_buf(),
        }
    }

    pub fn store(&self, payload: &[u8], signatures: Vec<Signer>) -> Result<(), std::io::Error> {
        log::debug!("Storing signatures in {}", self.signatures_file.to_string_lossy());
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.signatures_file)?;

        file.write_all(b"payload: ")?;
        file.write_all(payload)?;
        file.write_all(b"\n")?;
        for signer in signatures {
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
