use std::time::Duration;

use log::{debug, error, info};
use tokio::task::JoinHandle;

use crate::peer_discovery::{PeerDiscovery, PeerInfo};

//In case of error, the requester will switch to a 30 seconds interval.
const INTERVAL_ON_ERROR: Duration = Duration::from_secs(60);

//How many times the requester will retry to poll the peer discovery in case of error.
const MAX_FAILED_RETRY_COUNT: u32 = 10;

enum PeersRequesterState {
    Success,
    Failed,
}

/// This struct is responsible for polling the peer discovery trait.
pub(crate) struct PeersRequester {
    state: PeersRequesterState,
    interval: tokio::time::Interval,
    max_failed_retry_count: u32,
}

impl PeersRequester {
    pub fn new(duration: Duration, max_failed_retry_count: u32) -> Self {
        let interval = tokio::time::interval(duration);
        PeersRequester {
            state: PeersRequesterState::Success,
            interval,
            max_failed_retry_count,
        }
    }

    /// Starts to poll [PeerDiscovery] trait.
    ///
    /// If [PeerDiscovery::poll] returns an error, the requester will switch to a 30 seconds interval.
    /// After failing MAX_FAILED_RETRY_COUNT times, it switches back to normal interval and
    /// returns empty list of peers to [crate::network::libp2p::behaviours::peer_discovery::behaviour::Behaviour].
    pub(crate) async fn spawn_peer_requester<P: PeerDiscovery + 'static>(
        mut peer_discovery: P,
        discovery_channel_tx: tokio::sync::mpsc::UnboundedSender<Vec<PeerInfo>>,
    ) -> anyhow::Result<JoinHandle<()>> {
        let tx = discovery_channel_tx;

        let join_handle = tokio::spawn(async move {
            let mut requester =
                PeersRequester::new(peer_discovery.get_poll_interval(), MAX_FAILED_RETRY_COUNT);
            loop {
                requester.interval.tick().await;

                let result = peer_discovery.poll(tx.clone()).await;
                match requester.state {
                    PeersRequesterState::Success => {
                        if let Err(err) = result {
                            error!("Error while polling peer discovery: {}", err);

                            //In case of failure switch to faster interval.
                            //If MAX_FAILED_RETRY_COUNT is reached and poll still fails, return empty list of peers,
                            //therefore giving the Behaviour chance to react.

                            requester.max_failed_retry_count = MAX_FAILED_RETRY_COUNT;
                            requester.state = PeersRequesterState::Failed;
                            requester.interval = tokio::time::interval(INTERVAL_ON_ERROR);

                            info!("Trying {MAX_FAILED_RETRY_COUNT} times with {} interval to poll peer discovery", INTERVAL_ON_ERROR.as_secs());
                        }
                    }
                    PeersRequesterState::Failed => {
                        if result.is_ok() {
                            requester.state = PeersRequesterState::Success;
                            requester.interval =
                                tokio::time::interval(peer_discovery.get_poll_interval());
                        } else if requester.max_failed_retry_count == 0 {
                            debug!("Max failed retry count reached, switching to normal interval");
                            if let Err(err) = tx.send(vec![]) {
                                error!("Error while sending empty peer list: {}", err);
                            }
                        }
                        requester.max_failed_retry_count -= 1;
                    }
                };
            }
        });
        Ok(join_handle)
    }
}
