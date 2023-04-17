use std::{path::PathBuf, sync::Arc};

use clap::Parser;
use log::trace;
use tokio::{
    signal::unix::{signal, SignalKind},
    sync::mpsc::UnboundedSender,
};

use crate::{
    api::application::CheckBlockResult,
    cli::peers::ConfigPeers,
    codec::EphemeraEncoder,
    config::Configuration,
    core::builder::EphemeraStarter,
    crypto::EphemeraKeypair,
    crypto::Keypair,
    ephemera_api::{
        ApiBlock, ApiEphemeraMessage, Application, DummyApplication, RawApiEphemeraMessage, Result,
    },
    peer_discovery::{self, PeerDiscovery, PeerInfo},
    utilities::encoding::Encoder,
};

#[derive(Debug, Clone, Parser)]
pub struct RunExternalNodeCmd {
    #[clap(short, long)]
    pub config_file: String,
}

impl RunExternalNodeCmd {
    pub async fn execute(&self) -> anyhow::Result<()> {
        let conf = match Configuration::try_load(PathBuf::from(self.config_file.as_str())) {
            Ok(conf) => conf,
            Err(err) => anyhow::bail!("Error loading configuration file: {err:?}"),
        };

        let ephemera = EphemeraStarter::new(conf.clone())
            .unwrap()
            .with_application(DummyApplication)
            .with_peer_discovery(ConfigPeers::load()?)
            .init_tasks()
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
        Ok(())
    }
}

pub struct SignatureVerificationApplication {
    keypair: Arc<Keypair>,
}

impl SignatureVerificationApplication {
    pub fn new(keypair: Arc<Keypair>) -> Self {
        Self { keypair }
    }

    pub(crate) fn verify_message(&self, msg: ApiEphemeraMessage) -> anyhow::Result<()> {
        let signature = msg.certificate.clone();
        let raw_message: RawApiEphemeraMessage = msg.into();
        let encoded_message = Encoder::encode(&raw_message)?;
        if self.keypair.verify(&encoded_message, &signature.signature) {
            Ok(())
        } else {
            anyhow::bail!("Invalid signature")
        }
    }
}

impl Application for SignatureVerificationApplication {
    fn check_tx(&self, tx: ApiEphemeraMessage) -> Result<bool> {
        trace!("SignatureVerificationApplicationHook::check_tx");
        self.verify_message(tx)?;
        Ok(true)
    }

    fn check_block(&self, _block: &ApiBlock) -> Result<CheckBlockResult> {
        Ok(CheckBlockResult::Accept)
    }

    fn deliver_block(&self, _block: ApiBlock) -> Result<()> {
        trace!("SignatureVerificationApplicationHook::deliver_block");
        Ok(())
    }
}

#[async_trait::async_trait]
impl PeerDiscovery for ConfigPeers {
    async fn poll(
        &mut self,
        discovery_channel: UnboundedSender<Vec<PeerInfo>>,
    ) -> peer_discovery::Result<()> {
        let peer_info = self
            .peers
            .iter()
            .map(|peer| PeerInfo::try_from(peer.clone()))
            .collect::<anyhow::Result<Vec<PeerInfo>>>()?;

        discovery_channel.send(peer_info).unwrap();
        Ok(())
    }

    fn get_poll_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(60 * 60 * 24)
    }
}
