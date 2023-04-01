use std::sync::Arc;

use tokio::sync::broadcast::Sender;
use tokio::sync::oneshot::Receiver;
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;

use ephemera::api::EphemeraExternalApi;
use ephemera::config::Configuration;
use ephemera::crypto::{Ed25519Keypair, EphemeraKeypair, Keypair};
use ephemera::{EphemeraStarter, ShutdownHandle};
use metrics::MetricsCollector;

use crate::epoch::Epoch;
use crate::nym_api_ephemera::application::RewardsEphemeraApplication;
use crate::nym_api_ephemera::peer_discovery::HttpPeerDiscovery;
use crate::reward::new::aggregator::RewardsAggregator;
use crate::reward::{EphemeraAccess, RewardManager, V2};
use crate::storage::db::{MetricsStorageType, Storage};
use crate::{metrics, Args};

pub(crate) mod application;
pub(crate) mod peer_discovery;

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
        log::info!(
            "Starting nym api with ephemera {} ...",
            args.ephemera_config
        );
        //KEYPAIR - Ephemera keypair or Validator keypair
        //Can be a file, keystore etc
        let key_pair = Self::read_nym_api_keypair(&ephemera_config)?;
        let storage = Self::open_nym_api_storage(&args);

        //APPLICATION(ABCI) for Ephemera
        let rewards_ephemera_application =
            RewardsEphemeraApplication::new(ephemera_config.clone())?;
        let peer_discovery = HttpPeerDiscovery::new(args.smart_contract_url.clone());

        //EPHEMERA
        let ephemera_builder = EphemeraStarter::new(ephemera_config.clone())?;
        let ephemera_builder = ephemera_builder.with_application(rewards_ephemera_application);
        let ephemera_builder = ephemera_builder.with_peer_discovery(peer_discovery);
        let ephemera = ephemera_builder.init_tasks().await?;

        let mut ephemera_handle = ephemera.handle();

        //METRICS
        let metrics = Self::create_metrics_collector(&args, &storage);

        //REWARDS
        let rewards =
            Self::create_rewards_manager(args, key_pair, storage, ephemera_handle.api.clone())
                .await;

        //STARTING
        log::info!("Starting Nym-Api services");
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

        log::info!("Shut down complete");
        Ok(())
    }

    fn create_metrics_collector(
        args: &Args,
        storage: &Arc<Mutex<Storage<MetricsStorageType>>>,
    ) -> MetricsCollector {
        MetricsCollector::new(storage.clone(), args.metrics_collector_interval_seconds)
    }

    async fn create_rewards_manager(
        args: Args,
        key_pair: Ed25519Keypair,
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
        log::info!("Shutting down nym api ...");
        shutdown_signal_tx.send(())?;

        log::info!("Shutting down metrics collector ...");
        metrics.await?;
        log::info!("Metrics collector shut down complete");

        log::info!("Shutting down rewards ...");
        //doing abort here, rewards has unresponsive long-running loop to submit rewards.
        //No need to bother about graceful shutdown(in simulation)
        rewards.abort();
        log::info!("Rewards shut down complete");

        log::info!("Shutting down ephemera ...");
        ephemera_shutdown.shutdown();
        ephemera.await?;
        log::info!("Ephemera shut down complete");

        log::info!("Shut down complete...");

        Ok(())
    }

    fn open_nym_api_storage(args: &Args) -> Arc<Mutex<Storage<MetricsStorageType>>> {
        Arc::new(Mutex::new(Storage::new(
            args.metrics_db_path.clone(),
            migrations::migrations::runner(),
        )))
    }

    fn read_nym_api_keypair(ephemera_config: &Configuration) -> anyhow::Result<Ed25519Keypair> {
        let key_pair = bs58::decode(&ephemera_config.node.private_key).into_vec()?;
        let key_pair = Keypair::from_raw_vec(key_pair)?;
        Ok(key_pair)
    }
}
