use std::str::FromStr;
use std::sync::Arc;

use clap::{Args, Parser};
use log::trace;
use reqwest::Url;
use tokio::signal::unix::{signal, SignalKind};

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
    membership::MembersProviderFut,
    membership::{DummyMembersProvider, HttpMembersProvider},
    network::members::ConfigMembersProvider,
    utilities::encoding::Encoder,
};

#[derive(Clone, Debug)]
pub struct HttpMembersProviderArg {
    pub url: Url,
}

impl FromStr for HttpMembersProviderArg {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(HttpMembersProviderArg { url: s.parse()? })
    }
}

#[derive(Args)]
#[group(required = true, multiple = false)]
pub struct MembersProviderType {
    #[clap(short, long)]
    dummy_members_provider: Option<bool>,
    #[clap(short, long)]
    config_members_provider: Option<bool>,
    #[clap(short, long)]
    http_members_provider: Option<HttpMembersProviderArg>,
}

#[derive(Parser)]
pub struct RunExternalNodeCmd {
    #[clap(short, long)]
    pub config_file: String,
    #[command(flatten)]
    pub members_provider: MembersProviderType,
}

impl RunExternalNodeCmd {
    pub async fn execute(&self) -> anyhow::Result<()> {
        let ephemera_conf = match Configuration::try_load(self.config_file.clone()) {
            Ok(conf) => conf,
            Err(err) => anyhow::bail!("Error loading configuration file: {err:?}"),
        };

        let ephemera = EphemeraStarter::new(ephemera_conf.clone())
            .unwrap()
            .with_application(DummyApplication)
            .with_members_provider(self.members_provider()?)
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

    fn members_provider(&self) -> anyhow::Result<MembersProviderFut> {
        match &self.members_provider {
            MembersProviderType {
                dummy_members_provider: Some(true),
                ..
            } => Ok(Box::pin(DummyMembersProvider::empty_peers_list())),
            MembersProviderType {
                config_members_provider: Some(true),
                ..
            } => Ok(Box::pin(Self::config_members_provider()?)),
            MembersProviderType {
                http_members_provider: Some(HttpMembersProviderArg { url }),
                ..
            } => Ok(Box::pin(Self::http_members_provider(url.to_string())?)),
            _ => anyhow::bail!("Invalid members provider"),
        }
    }

    fn config_members_provider() -> anyhow::Result<ConfigMembersProvider> {
        let peers_conf_path = Configuration::ephemera_root_dir()
            .unwrap()
            .join(PEERS_CONFIG_FILE);

        let peers_conf = match ConfigMembersProvider::init(peers_conf_path) {
            Ok(conf) => conf,
            Err(err) => anyhow::bail!("Error loading peers file: {err:?}"),
        };
        Ok(peers_conf)
    }

    fn http_members_provider(url: String) -> anyhow::Result<HttpMembersProvider> {
        let http_members_provider = HttpMembersProvider::new(url);
        Ok(http_members_provider)
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
