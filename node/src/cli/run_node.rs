use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use log::trace;
use tokio::signal::unix::{signal, SignalKind};

use crate::peer_discovery::HttpPeerDiscovery;
use crate::{
    api::application::CheckBlockResult,
    cli::PEERS_CONFIG_FILE,
    codec::EphemeraEncoder,
    config::Configuration,
    core::builder::EphemeraStarter,
    crypto::EphemeraKeypair,
    crypto::Keypair,
    ephemera_api::{
        ApiBlock, ApiEphemeraMessage, Application, DummyApplication, RawApiEphemeraMessage, Result,
    },
    network::peer_discovery::ConfigPeerDiscovery,
    utilities::encoding::Encoder,
};

#[derive(Debug, Clone, Parser)]
pub struct RunExternalNodeCmd {
    #[clap(short, long)]
    pub config_file: String,
}

impl RunExternalNodeCmd {
    pub async fn execute(&self) -> anyhow::Result<()> {
        let ephemera_conf = match Configuration::try_load(self.config_file.clone()) {
            Ok(conf) => conf,
            Err(err) => anyhow::bail!("Error loading configuration file: {err:?}"),
        };

        let config_peer_discovery = Self::config_peer_discovery()?;
        let http_peer_discovery = Self::http_peer_discovery(
            "http://localhost:8000/peers".to_string(),
            Duration::from_secs(60),
        )?;

        let ephemera = EphemeraStarter::new(ephemera_conf.clone())
            .unwrap()
            .with_application(DummyApplication)
            .with_peer_discovery(http_peer_discovery)
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

    fn config_peer_discovery() -> anyhow::Result<ConfigPeerDiscovery> {
        let peers_conf_path = Configuration::ephemera_root_dir()
            .unwrap()
            .join(PEERS_CONFIG_FILE);

        let peers_conf =
            match ConfigPeerDiscovery::init(peers_conf_path, Duration::from_secs(60 * 60 * 24)) {
                Ok(conf) => conf,
                Err(err) => anyhow::bail!("Error loading peers file: {err:?}"),
            };
        Ok(peers_conf)
    }

    fn http_peer_discovery(
        url: String,
        reload_interval: Duration,
    ) -> anyhow::Result<HttpPeerDiscovery> {
        let http_peer_discovery = HttpPeerDiscovery::new(url, reload_interval);
        Ok(http_peer_discovery)
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
