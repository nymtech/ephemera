use std::num::NonZeroUsize;

use log::{debug, error, trace};
use lru::LruCache;
use tokio::sync::oneshot::Sender;

use crate::api::types::ApiBroadcastInfo;
use crate::api::{DhtKV, DhtKey, DhtValue};
use crate::ephemera_api::ApiEphemeraMessage;
use crate::{
    api::{
        self,
        application::Application,
        types::{ApiBlock, ApiCertificate, ApiError},
        ToEphemeraApiCmd,
    },
    block::{manager::BlockManagerError, types::message},
    crypto::EphemeraKeypair,
    ephemera_api::ApiEphemeraConfig,
    network::libp2p::ephemera_sender::EphemeraEvent,
    Ephemera,
};

type DhtPendingQueryReply = Sender<Result<Option<(Vec<u8>, Vec<u8>)>, ApiError>>;

pub(crate) struct ApiCmdProcessor {
    pub(crate) dht_query_cache: LruCache<Vec<u8>, DhtPendingQueryReply>,
}

impl ApiCmdProcessor {
    pub(crate) fn new() -> Self {
        Self {
            dht_query_cache: LruCache::new(NonZeroUsize::new(1000).unwrap()),
        }
    }

    pub(crate) async fn process_api_requests<A: Application>(
        ephemera: &mut Ephemera<A>,
        cmd: ToEphemeraApiCmd,
    ) -> api::Result<()> {
        trace!("Processing API request: {:?}", cmd);
        match cmd {
            ToEphemeraApiCmd::SubmitEphemeraMessage(api_msg, reply) => {
                // Ask application to decide if we should accept this message.
                Self::submit_message(ephemera, api_msg, reply).await?;
            }

            ToEphemeraApiCmd::QueryBlockById(block_id, reply) => {
                Self::query_block_by_id(ephemera, &block_id, reply).await;
            }

            ToEphemeraApiCmd::QueryBlockByHeight(height, reply) => {
                Self::query_block_by_height(ephemera, height, reply).await;
            }

            ToEphemeraApiCmd::QueryLastBlock(reply) => {
                Self::query_last_block(ephemera, reply).await;
            }

            ToEphemeraApiCmd::QueryBlockCertificates(block_id, reply) => {
                Self::query_block_certificates(ephemera, &block_id, reply).await;
            }

            ToEphemeraApiCmd::QueryDht(key, reply) => {
                Self::query_dht(ephemera, key, reply).await;
            }

            ToEphemeraApiCmd::StoreInDht(key, value, reply) => {
                Self::store_in_dht(ephemera, key, value, reply).await;
            }

            ToEphemeraApiCmd::EphemeraConfig(reply) => {
                Self::ephemera_config(ephemera, reply);
            }

            ToEphemeraApiCmd::BroadcastGroup(reply) => {
                Self::broadcast_group(ephemera, reply);
            }
        }
        Ok(())
    }

    fn broadcast_group<A: Application>(
        ephemera: &mut Ephemera<A>,
        reply: Sender<api::Result<ApiBroadcastInfo>>,
    ) {
        let group_peers = ephemera.broadcast_group();

        let bc = ApiBroadcastInfo::new(group_peers.clone(), ephemera.node_info.peer_id);
        reply
            .send(Ok(bc))
            .expect("Error sending BroadcastGroup response to api");
    }

    fn ephemera_config<A: Application>(
        ephemera: &mut Ephemera<A>,
        reply: Sender<api::Result<ApiEphemeraConfig>>,
    ) {
        let node_info = ephemera.node_info.clone();
        let api_config = ApiEphemeraConfig {
            protocol_address: node_info.protocol_address(),
            api_address: node_info.api_address_http(),
            websocket_address: node_info.ws_address_ws(),
            public_key: node_info.keypair.public_key().to_string(),
            block_producer: node_info.initial_config.block.producer,
            block_creation_interval_sec: node_info.initial_config.block.creation_interval_sec,
        };
        reply
            .send(Ok(api_config))
            .expect("Error sending EphemeraConfig response to api");
    }

    async fn store_in_dht<A: Application>(
        ephemera: &mut Ephemera<A>,
        key: DhtKey,
        value: DhtValue,
        reply: Sender<api::Result<()>>,
    ) {
        let response = match ephemera
            .to_network
            .send_ephemera_event(EphemeraEvent::StoreInDht { key, value })
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => Err(ApiError::Internal(err)),
        };
        reply
            .send(response)
            .expect("Error sending StoreInDht response to api");
    }

    async fn query_dht<A: Application>(
        ephemera: &mut Ephemera<A>,
        key: DhtKey,
        reply: Sender<api::Result<Option<DhtKV>>>,
    ) {
        //FIXME: This is very loose, we could have multiple pending queries for the same key
        match ephemera
            .to_network
            .send_ephemera_event(EphemeraEvent::QueryDht { key: key.clone() })
            .await
        {
            Ok(_) => {
                //Save the reply channel in a map and send the reply when we get the response from the network
                ephemera
                    .api_cmd_processor
                    .dht_query_cache
                    .put(key.clone(), reply);
            }
            Err(err) => {
                error!("Error sending QueryDht to network: {:?}", err);
                reply
                    .send(Err(ApiError::Internal(err)))
                    .expect("Error sending QueryDht response to api");
            }
        };
    }

    async fn query_block_certificates<A: Application>(
        ephemera: &mut Ephemera<A>,
        block_id: &str,
        reply: Sender<api::Result<Option<Vec<ApiCertificate>>>>,
    ) {
        let response = match ephemera
            .storage
            .lock()
            .await
            .get_block_certificates(block_id)
        {
            Ok(signatures) => {
                let certificates = signatures.map(|s| {
                    s.into_iter()
                        .map(Into::into)
                        .collect::<Vec<ApiCertificate>>()
                });
                Ok(certificates)
            }
            Err(err) => Err(ApiError::Internal(err)),
        };
        reply
            .send(response)
            .expect("Error sending QueryBlockSignatures response to api");
    }

    async fn query_last_block<A: Application>(
        ephemera: &mut Ephemera<A>,
        reply: Sender<api::Result<ApiBlock>>,
    ) {
        let response = match ephemera.storage.lock().await.get_last_block() {
            Ok(Some(block)) => Ok(block.into()),
            Ok(None) => Err(ApiError::Internal(anyhow::Error::msg(
                "No blocks found, this is a bug!",
            ))),
            Err(err) => Err(ApiError::Internal(err)),
        };
        reply
            .send(response)
            .expect("Error sending QueryLastBlock response to api");
    }

    async fn query_block_by_height<A: Application>(
        ephemera: &mut Ephemera<A>,
        height: u64,
        reply: Sender<api::Result<Option<ApiBlock>>>,
    ) {
        let response = match ephemera.storage.lock().await.get_block_by_height(height) {
            Ok(Some(block)) => {
                let api_block: ApiBlock = block.into();
                Ok(api_block.into())
            }
            Ok(None) => Ok(None),
            Err(err) => Err(ApiError::Internal(err)),
        };
        reply
            .send(response)
            .expect("Error sending QueryBlockByHeight response to api");
    }

    async fn query_block_by_id<A: Application>(
        ephemera: &mut Ephemera<A>,
        block_id: &str,
        reply: Sender<api::Result<Option<ApiBlock>>>,
    ) {
        let response = match ephemera.storage.lock().await.get_block_by_id(block_id) {
            Ok(Some(block)) => {
                let api_block: ApiBlock = block.into();
                Ok(api_block.into())
            }
            Ok(None) => Ok(None),
            Err(err) => Err(ApiError::Internal(err)),
        };
        reply
            .send(response)
            .expect("Error sending QueryBlockById response to api");
    }

    async fn submit_message<A: Application>(
        ephemera: &mut Ephemera<A>,
        api_msg: Box<ApiEphemeraMessage>,
        reply: Sender<api::Result<()>>,
    ) -> api::Result<()> {
        let response = match ephemera.application.check_tx(*api_msg.clone()) {
            Ok(true) => {
                trace!("Application accepted ephemera message: {:?}", api_msg);

                // Send to BlockManager to verify it and put into memory pool
                let ephemera_msg: message::EphemeraMessage = (*api_msg).into();
                match ephemera.block_manager.on_new_message(ephemera_msg.clone()) {
                    Ok(_) => {
                        //Gossip to network for other nodes to receive
                        match ephemera
                            .to_network
                            .send_ephemera_event(EphemeraEvent::EphemeraMessage(
                                ephemera_msg.into(),
                            ))
                            .await
                        {
                            Ok(_) => Ok(()),
                            Err(err) => Err(ApiError::Internal(err)),
                        }
                    }
                    Err(err) => match err {
                        BlockManagerError::DuplicateMessage(_) => Err(ApiError::DuplicateMessage),
                        BlockManagerError::BlockManager(err) => {
                            Err(ApiError::Internal(anyhow::Error::msg(err.to_string())))
                        }
                    },
                }
            }
            Ok(false) => {
                debug!("Application rejected ephemera message: {:?}", api_msg);
                Err(ApiError::ApplicationRejectedMessage)
            }
            Err(err) => {
                error!("Application rejected ephemera message: {:?}", err);
                Err(ApiError::Application(err))
            }
        };
        reply
            .send(response)
            .expect("Error sending SubmitEphemeraMessage response to api");
        Ok(())
    }
}
