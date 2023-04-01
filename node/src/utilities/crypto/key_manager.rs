use std::sync::Arc;

use crate::utilities::encoding::from_base58;
use crate::utilities::{Ed25519Keypair, EphemeraKeypair};

pub(crate) struct KeyManager;

impl KeyManager {
    //FIXME: works for dev cluster for now
    pub(crate) fn read_keypair(private_key: String) -> anyhow::Result<Arc<Ed25519Keypair>> {
        let keypair = from_base58(private_key).map_err(|e| {
            anyhow::anyhow!("Failed to parse private key from config. Error: {e:?}")
        })?;
        let keypair: Arc<Ed25519Keypair> = Ed25519Keypair::from_raw_vec(keypair)?.into();
        Ok(keypair)
    }
}
