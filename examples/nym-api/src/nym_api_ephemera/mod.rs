use std::sync::Arc;
use std::time::Duration;

use log::info;
use tokio::sync::broadcast::Sender;
use tokio::sync::oneshot::Receiver;
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;

use ephemera::configuration::Configuration;
use ephemera::crypto::{EphemeraKeypair, Keypair};
use ephemera::ephemera_api::EphemeraExternalApi;
use ephemera::membership::HttpMembersProvider;
use ephemera::{Ephemera, EphemeraStarter, ShutdownHandle};
use metrics::MetricsCollector;

use crate::epoch::Epoch;
use crate::nym_api_ephemera::application::RewardsEphemeraApplication;
use crate::reward::new::aggregator::RewardsAggregator;
use crate::reward::{EphemeraAccess, RewardManager, V2};
use crate::storage::db::{MetricsStorageType, Storage};
use crate::{metrics, Args};

pub(crate) mod application;

mod migrations {
    use refinery::embed_migrations;

    embed_migrations!("migrations/rewardsnew");
}

pub struct NymApi;

impl NymApi {
    pub async fn run(
        args: Args,
        ephemera_config: Configuration,
        shutdown: Receiver<()>,
    ) -> anyhow::Result<()> {
        info!(
            "Starting nym api with ephemera {} ...",
            args.ephemera_config
        );
        //KEYPAIR - Ephemera keypair or Validator keypair
        //Can be a file, keystore etc
        let key_pair = Self::read_nym_api_keypair(&ephemera_config)?;
        let storage = Self::open_nym_api_storage(&args);

        //EPHEMERA
        let ephemera = Self::init_ephemera(&args, ephemera_config).await?;
        let mut ephemera_handle = ephemera.handle();

        //METRICS
        let metrics = Self::create_metrics_collector(&args, &storage);

        //REWARDS
        let rewards =
            Self::create_rewards_manager(args, key_pair, storage, ephemera_handle.api.clone())
                .await;

        //STARTING
        info!("Starting Nym-Api services");
        let (shutdown_signal_tx, _shutdown_signal_rcv) = broadcast::channel(1);
        let ephemera_task = tokio::spawn(ephemera.run());
        let rewards_task = tokio::spawn(rewards.start(shutdown_signal_tx.subscribe()));
        let metrics_task = tokio::spawn(metrics.start(shutdown_signal_tx.subscribe()));

        //SHUTDOWN
        Self::shutdown_nym_api(
            shutdown,
            &mut ephemera_handle.shutdown,
            shutdown_signal_tx,
            ephemera_task,
            rewards_task,
            metrics_task,
        )
        .await?;

        info!("Shut down complete");
        Ok(())
    }

    async fn init_ephemera(
        args: &Args,
        ephemera_config: Configuration,
    ) -> anyhow::Result<Ephemera<RewardsEphemeraApplication>> {
        info!("Initializing ephemera ...");

        //Application for Ephemera
        let rewards_ephemera_application =
            RewardsEphemeraApplication::init(ephemera_config.clone())?;

        //Members provider for Ephemera
        let url = format!("http://{}/contract/peer_info", args.smart_contract_url);
        let members_provider = HttpMembersProvider::new(url);

        //EPHEMERA
        let ephemera_builder = EphemeraStarter::new(ephemera_config.clone())?;
        let ephemera_builder = ephemera_builder.with_application(rewards_ephemera_application);
        let ephemera_builder = ephemera_builder.with_members_provider(Box::pin(members_provider));
        let ephemera = ephemera_builder.init_tasks().await?;
        Ok(ephemera)
    }

    fn create_metrics_collector(
        args: &Args,
        storage: &Arc<Mutex<Storage<MetricsStorageType>>>,
    ) -> MetricsCollector {
        MetricsCollector::new(storage.clone(), args.metrics_collector_interval_seconds)
    }

    async fn create_rewards_manager(
        args: Args,
        key_pair: Keypair,
        storage: Arc<Mutex<Storage<MetricsStorageType>>>,
        ephemera_api: EphemeraExternalApi,
    ) -> RewardManager<V2> {
        let epoch = Epoch::request_epoch(args.smart_contract_url.clone()).await;
        let rewards: RewardManager<V2> = RewardManager::new(
            storage.clone(),
            args.clone(),
            EphemeraAccess::new(ephemera_api, key_pair).into(),
            Some(RewardsAggregator),
            epoch,
        );
        rewards
    }

    async fn shutdown_nym_api(
        shutdown: Receiver<()>,
        ephemera_shutdown: &mut ShutdownHandle,
        shutdown_signal_tx: Sender<()>,
        ephemera: JoinHandle<()>,
        rewards: JoinHandle<()>,
        metrics: JoinHandle<()>,
    ) -> anyhow::Result<()> {
        shutdown.await?;
        info!("Shutting down nym api ...");
        shutdown_signal_tx.send(())?;

        info!("Shutting down metrics collector ...");
        metrics.await?;
        info!("Metrics collector shut down complete");

        info!("Shutting down rewards ...");
        //doing abort here, rewards has unresponsive long-running loop to submit rewards.
        //No need to bother about graceful shutdown(in simulation)
        rewards.abort();
        info!("Rewards shut down complete");

        info!("Shutting down ephemera ...");
        ephemera_shutdown.shutdown();
        ephemera.await?;
        info!("Ephemera shut down complete");

        info!("Shut down complete...");

        Ok(())
    }

    fn open_nym_api_storage(args: &Args) -> Arc<Mutex<Storage<MetricsStorageType>>> {
        Arc::new(Mutex::new(Storage::init(
            args.metrics_db_path.clone(),
            migrations::migrations::runner(),
        )))
    }

    fn read_nym_api_keypair(ephemera_config: &Configuration) -> anyhow::Result<Keypair> {
        let key_pair = bs58::decode(&ephemera_config.node.private_key).into_vec()?;
        let key_pair = Keypair::from_raw_vec(key_pair)?;
        Ok(key_pair)
    }
}
