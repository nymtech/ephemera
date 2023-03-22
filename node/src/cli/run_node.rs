use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use tokio::signal::unix::{signal, SignalKind};

use crate::api::application::{Application, DefaultApplication};
use crate::api::types::{ApiBlock, ApiEphemeraMessage, ApiEphemeraRawMessage};
use crate::config::{Configuration, Libp2pConfig, PeerSetting};
use crate::core::builder::EphemeraStarter;
use crate::utilities::crypto::ed25519::Ed25519Keypair;
use crate::utilities::crypto::keypair::Keypair;
use crate::utilities::encode;

#[derive(Debug, Clone, Parser)]
pub struct RunExternalNodeCmd {
    #[clap(short, long)]
    pub config_file: String,
}

struct DummyPeerDiscovery;

use crate::network::{PeerDiscovery, PeerInfo};
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;

#[async_trait]
impl PeerDiscovery for DummyPeerDiscovery {
    async fn poll(&mut self, _: UnboundedSender<Vec<PeerInfo>>) -> anyhow::Result<()> {
        Ok(())
    }

    fn get_request_interval_in_sec(&self) -> u64 {
        0
    }
}

impl RunExternalNodeCmd {
    pub async fn execute(&self) -> anyhow::Result<()> {
        let conf = match Configuration::try_load(PathBuf::from(self.config_file.as_str())) {
            Ok(conf) => conf,
            Err(err) => anyhow::bail!("Error loading configuration file: {err:?}"),
        };

        let ephemera = EphemeraStarter::new(conf.clone())
            .unwrap()
            .with_application(DefaultApplication)
            .with_peer_discovery(ConfigPeers::new(&conf.libp2p))
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

#[derive(Debug, Clone)]
pub(crate) struct ConfigPeers {
    config: Libp2pConfig,
}

impl ConfigPeers {
    pub fn new(config: &Libp2pConfig) -> ConfigPeers {
        ConfigPeers {
            config: config.clone(),
        }
    }
}

impl From<PeerSetting> for PeerInfo {
    fn from(setting: PeerSetting) -> Self {
        PeerInfo {
            name: setting.name,
            address: setting.address,
            pub_key: setting.pub_key,
        }
    }
}

#[async_trait::async_trait]
impl PeerDiscovery for ConfigPeers {
    async fn poll(
        &mut self,
        discovery_channel: UnboundedSender<Vec<PeerInfo>>,
    ) -> anyhow::Result<()> {
        let mut peers = vec![];
        for setting in self.config.peers.clone() {
            peers.push(setting.into());
        }
        discovery_channel.send(peers).unwrap();
        Ok(())
    }

    fn get_request_interval_in_sec(&self) -> u64 {
        u64::MAX
    }
}
