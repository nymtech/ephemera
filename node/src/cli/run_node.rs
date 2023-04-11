use std::path::PathBuf;
use std::sync::Arc;

use crate::api::application::CheckBlockResult;
use async_trait::async_trait;
use clap::Parser;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc::UnboundedSender;

use crate::codec::EphemeraEncoder;
use crate::config::{Configuration, PeerSetting};
use crate::core::builder::EphemeraStarter;
use crate::crypto::{EphemeraKeypair, EphemeraPublicKey, Keypair, PublicKey};
use crate::ephemera_api::{
    ApiBlock, ApiEphemeraMessage, Application, DefaultApplication, RawApiEphemeraMessage, Result,
};
use crate::network::discovery::{PeerDiscovery, PeerInfo};
use crate::peer_discovery;
use crate::utilities::encoding::Encoder;

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
            .with_application(DefaultApplication)
            .with_peer_discovery(ConfigPeers::new(&conf))
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
        log::trace!("SignatureVerificationApplicationHook::check_tx");
        self.verify_message(tx)?;
        Ok(true)
    }

    fn check_block(&self, _block: &ApiBlock) -> Result<CheckBlockResult> {
        Ok(CheckBlockResult::Accept)
    }

    fn deliver_block(&self, _block: ApiBlock) -> Result<()> {
        log::trace!("SignatureVerificationApplicationHook::deliver_block");
        Ok(())
    }
}

struct DummyPeerDiscovery;

#[async_trait]
impl PeerDiscovery for DummyPeerDiscovery {
    async fn poll(&mut self, _: UnboundedSender<Vec<PeerInfo>>) -> peer_discovery::Result<()> {
        Ok(())
    }

    fn get_poll_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(60 * 60 * 24)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ConfigPeers {
    peers: Vec<PeerInfo>,
}

impl ConfigPeers {
    pub fn new(config: &Configuration) -> ConfigPeers {
        let mut peers = vec![];
        for setting in config.libp2p.peers.clone() {
            peers.push(setting.try_into().unwrap());
        }

        //Construct local peer info
        let private_key = Keypair::from_base58(config.node.private_key.as_str()).unwrap();
        let public_key = private_key.public_key().to_base58();
        let local = PeerSetting {
            name: "local".to_string(),
            address: format!("/ip4/{}/tcp/{}", config.node.ip, config.libp2p.port,),
            pub_key: public_key,
        };
        peers.push(local.try_into().unwrap());

        ConfigPeers { peers }
    }
}

impl TryFrom<PeerSetting> for PeerInfo {
    type Error = anyhow::Error;

    fn try_from(setting: PeerSetting) -> std::result::Result<Self, Self::Error> {
        let pub_key = PublicKey::from_base58(setting.pub_key.as_str())?;
        Ok(PeerInfo {
            name: setting.name,
            address: setting.address,
            pub_key,
        })
    }
}

#[async_trait::async_trait]
impl PeerDiscovery for ConfigPeers {
    async fn poll(
        &mut self,
        discovery_channel: UnboundedSender<Vec<PeerInfo>>,
    ) -> peer_discovery::Result<()> {
        discovery_channel.send(self.peers.clone()).unwrap();
        Ok(())
    }

    fn get_poll_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(60 * 60 * 24)
    }
}
