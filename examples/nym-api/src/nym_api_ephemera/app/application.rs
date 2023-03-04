use ephemera::api::application::Application;
use ephemera::api::types::{ApiBlock, ApiEphemeraMessage};
use ephemera::config::Configuration;
use ephemera::utilities::{decode, Ed25519PublicKey, PublicKey, ToPeerId};

use crate::contract::MixnodeToReward;
use crate::nym_api_ephemera::app::nym_peers::NymApiEphemeraPeerInfo;

pub(crate) struct RewardsEphemeraApplicationConfig {
    /// Percentage of messages relative to total number of peers
    pub(crate) peers_rewards_threshold: u64,
}

pub(crate) struct RewardsEphemeraApplication {
    peer_info: NymApiEphemeraPeerInfo,
    app_config: RewardsEphemeraApplicationConfig,
}

impl RewardsEphemeraApplication {
    pub(crate) fn new(ephemera_config: Configuration) -> anyhow::Result<Self> {
        let peer_info = NymApiEphemeraPeerInfo::from_ephemera_dev_cluster_conf(&ephemera_config)?;
        let app_config = RewardsEphemeraApplicationConfig {
            peers_rewards_threshold: peer_info.get_peers_count() as u64,
        };
        Ok(Self {
            peer_info,
            app_config,
        })
    }
}

/// - TODO: We should also check that the messages has expected label(like epoch 100)
///         because next block should have only reward info for correct epoch.
impl Application for RewardsEphemeraApplication {
    /// Perform validation checks:
    /// - Check that the transaction has a valid signature, we don't want to accept garbage messages
    ///   or messages from unknown peers
    fn check_tx(&self, tx: ApiEphemeraMessage) -> anyhow::Result<bool> {
        if decode::<Vec<MixnodeToReward>>(&tx.data).is_err() {
            log::error!("Message is not a valid Reward message");
            return Ok(false);
        }

        let public_key = Ed25519PublicKey::from_raw_vec(tx.signature.public_key)?;
        let peer_id = public_key.peer_id();

        match self.peer_info.get_peer_by_id(&peer_id) {
            None => {
                log::warn!("Unknown peer with public key: {}", peer_id);
                Ok(false)
            }
            Some(pk) => {
                if peer_id == self.peer_info.local_peer.peer_id {
                    log::debug!("Message signed by local peer: {}", peer_id);
                    return Ok(true);
                } else {
                    log::debug!(
                        "Message signed by peer: {}, address:{}",
                        peer_id,
                        pk.address
                    );
                }
                Ok(true)
            }
        }

        //TODO - check signatures
        //PS! message label should also be part of the signature to prevent replay attacks
        // let signature = tx.signature.clone();
        // let signature_bytes = from_hex(signature.signature)?;
        // public_key.verify(&message.data, &signature_bytes)?;
    }

    /// Agree to accept the block if it contains threshold number of transactions
    /// We trust that transactions are valid(checked by check_tx)
    fn accept_block(&self, block: &ApiBlock) -> anyhow::Result<bool> {
        log::debug!("Block message count: {}", block.message_count());

        let block_threshold = ((block.message_count() as f64
            / self.peer_info.get_peers_count() as f64)
            * 100.0) as u64;

        if block_threshold > 100 {
            log::error!("Block threshold is greater than 100%!. We expected only single message from each peer");
            return Ok(false);
        }

        if block_threshold >= self.app_config.peers_rewards_threshold {
            log::debug!("Block accepted");
            Ok(true)
        } else {
            log::debug!("Block rejected: not enough messages");
            Ok(false)
        }
    }

    /// It is possible to use this method as a callback to get notified when block is committed
    fn deliver_block(&self, _block: ApiBlock) -> anyhow::Result<()> {
        Ok(())
    }
}
