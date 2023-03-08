use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use tokio::signal::unix::{signal, SignalKind};

use crate::api::application::{Application, DefaultApplication};
use crate::api::types::{ApiBlock, ApiEphemeraMessage, ApiEphemeraRawMessage};
use crate::config::Configuration;
use crate::core::builder::EphemeraStarter;
use crate::utilities::crypto::ed25519::Ed25519Keypair;
use crate::utilities::crypto::keypair::Keypair;
use crate::utilities::encode;

#[derive(Debug, Clone, Parser)]
pub struct RunExternalNodeCmd {
    #[clap(short, long)]
    pub config_file: String,
}

impl RunExternalNodeCmd {
    pub async fn execute(&self) {
        let conf = match Configuration::try_load(PathBuf::from(self.config_file.as_str())) {
            Ok(conf) => conf,
            Err(err) => panic!("Error loading configuration file: {err:?}",),
        };

        let ephemera = EphemeraStarter::new(conf)
            .unwrap()
            .init_tasks(DefaultApplication)
            .await
            .unwrap();

        let mut ephemera_shutdown = ephemera.ephemera_handle.shutdown.clone();

        let ephemera_handle = tokio::spawn(ephemera.run());

        let shutdown = async {
            let mut stream_int = signal(SignalKind::interrupt()).unwrap();
            let mut stream_term = signal(SignalKind::terminate()).unwrap();
            tokio::select! {
                _ = stream_int.recv() => {
                    ephemera_shutdown.shutdown();
                }
                _ = stream_term.recv() => {
                   ephemera_shutdown.shutdown();
                }
            }
        };

        //Wait shutdown signal
        shutdown.await;
        ephemera_handle.await.unwrap();
    }
}

pub struct SignatureVerificationApplication {
    keypair: Arc<Ed25519Keypair>,
}

impl SignatureVerificationApplication {
    pub fn new(keypair: Arc<Ed25519Keypair>) -> Self {
        Self { keypair }
    }

    pub(crate) fn verify_message(&self, msg: ApiEphemeraMessage) -> anyhow::Result<()> {
        let signature = msg.signature.clone();
        let raw_message: ApiEphemeraRawMessage = msg.into();
        let encoded_message = encode(raw_message)?;
        if self.keypair.verify(&encoded_message, &signature.signature) {
            Ok(())
        } else {
            anyhow::bail!("Invalid signature")
        }
    }
}

impl Application for SignatureVerificationApplication {
    fn check_tx(&self, tx: ApiEphemeraMessage) -> anyhow::Result<bool> {
        log::trace!("SignatureVerificationApplicationHook::check_tx");
        self.verify_message(tx)?;
        Ok(true)
    }

    fn accept_block(&self, _block: &ApiBlock) -> anyhow::Result<bool> {
        todo!()
    }

    fn deliver_block(&self, _block: ApiBlock) -> anyhow::Result<()> {
        log::trace!("SignatureVerificationApplicationHook::deliver_block");
        Ok(())
    }
}
