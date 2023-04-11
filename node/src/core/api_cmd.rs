use std::num::NonZeroUsize;

use lru::LruCache;
use tokio::sync::oneshot::Sender;

use crate::{
    api::{
        self,
        ApiCmd,
        application::Application,
        types::{ApiBlock, ApiCertificate},
    },
    block::{manager::BlockManagerError, types::message},
    crypto::EphemeraKeypair,
    Ephemera,
    ephemera_api::ApiEphemeraConfig,
    network::libp2p::ephemera_sender::EphemeraEvent,
};
use crate::ephemera_api::ApiEphemeraMessage;

pub(crate) struct ApiCmdProcessor {
    pub(crate) dht_query_cache:
    LruCache<Vec<u8>, Sender<Result<Option<(Vec<u8>, Vec<u8>)>, api::ApiError>>>,
}

impl ApiCmdProcessor {
    pub(crate) fn new() -> Self {
        Self {
            dht_query_cache: LruCache::new(NonZeroUsize::new(1000).unwrap()),
        }
    }

    pub(crate) async fn process_api_requests<A: Application>(
        ephemera: &mut Ephemera<A>,
        cmd: ApiCmd,
    ) -> anyhow::Result<()> {
        log::debug!("Processing API request: {:?}", cmd);
        match cmd {
            ApiCmd::SubmitEphemeraMessage(api_msg, reply) => {
                // Ask application to decide if we should accept this message.
                Self::handle_submit_message(ephemera, api_msg, reply).await?;
            }

            ApiCmd::QueryBlockById(block_id, reply) => {
                let response = match ephemera.storage.lock().await.get_block_by_id(block_id) {
                    Ok(Some(block)) => {
                        let api_block: ApiBlock = block.into();
                        Ok(api_block.into())
                    }
                    Ok(None) => Ok(None),
                    Err(err) => Err(api::ApiError::Internal(err)),
                };
                reply
                    .send(response)
                    .expect("Error sending QueryBlockById response to api");
            }

            ApiCmd::QueryBlockByHeight(height, reply) => {
                let response = match ephemera.storage.lock().await.get_block_by_height(height) {
                    Ok(Some(block)) => {
                        let api_block: ApiBlock = block.into();
                        Ok(api_block.into())
                    }
                    Ok(None) => Ok(None),
                    Err(err) => Err(api::ApiError::Internal(err)),
                };
                reply
                    .send(response)
                    .expect("Error sending QueryBlockByHeight response to api");
            }

            ApiCmd::QueryLastBlock(reply) => {
                let response = match ephemera.storage.lock().await.get_last_block() {
                    Ok(Some(block)) => Ok(block.into()),
                    Ok(None) => Err(api::ApiError::Internal(anyhow::Error::msg(
                        "No blocks found, this is a bug!",
                    ))),
                    Err(err) => Err(api::ApiError::Internal(err)),
                };
                reply
                    .send(response)
                    .expect("Error sending QueryLastBlock response to api");
            }
            ApiCmd::QueryBlockCertificates(block_id, reply) => {
                let response = match ephemera
                    .storage
                    .lock()
                    .await
                    .get_block_certificates(block_id)
                {
                    Ok(signatures) => {
                        let certificates = signatures.map(|s| {
                            s.into_iter()
                                .map(|s| s.into())
                                .collect::<Vec<ApiCertificate>>()
                        });
                        Ok(certificates)
                    }
                    Err(err) => Err(api::ApiError::Internal(err)),
                };
                reply
                    .send(response)
                    .expect("Error sending QueryBlockSignatures response to api");
            }
            ApiCmd::QueryDht(key, reply) => {
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
                        return Ok(());
                    }
                    Err(err) => {
                        log::error!("Error sending QueryDht to network: {:?}", err);
                        reply
                            .send(Err(api::ApiError::Internal(err)))
                            .expect("Error sending QueryDht response to api");
                    }
                };
            }
            ApiCmd::StoreInDht(key, value, reply) => {
                let response = match ephemera
                    .to_network
                    .send_ephemera_event(EphemeraEvent::StoreInDht { key, value })
                    .await {
                    Ok(_) => Ok(()),
                    Err(err) => Err(api::ApiError::Internal(err)),
                };
                reply
                    .send(response)
                    .expect("Error sending StoreInDht response to api");
            }

            ApiCmd::EphemeraConfig(reply) => {
                let node_info = ephemera.node_info.clone();
                let api_config = ApiEphemeraConfig {
                    protocol_address: node_info.protocol_address(),
                    api_address: node_info.api_address_http(),
                    websocket_address: node_info.ws_address_ws(),
                    public_key: node_info.keypair.public_key().to_string(),
                    block_producer: node_info.initial_config.block.producer,
                    block_creation_interval_sec: node_info
                        .initial_config
                        .block
                        .creation_interval_sec,
                };
                reply
                    .send(Ok(api_config))
                    .expect("Error sending EphemeraConfig response to api");
            }
        }
        Ok(())
    }

    async fn handle_submit_message<A: Application>(
        ephemera: &mut Ephemera<A>,
        api_msg: Box<ApiEphemeraMessage>,
        reply: Sender<api::Result<()>>,
    ) -> anyhow::Result<()> {
        let response = match ephemera.application.check_tx(*api_msg.clone()) {
            Ok(true) => {
                log::trace!("Application accepted ephemera message: {:?}", api_msg);

                // Send to BlockManager to verify it and put into memory pool
                let ephemera_msg: message::EphemeraMessage = (*api_msg).try_into()?;
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
                            Err(err) => Err(api::ApiError::Internal(err)),
                        }
                    }
                    Err(err) => match err {
                        BlockManagerError::DuplicateMessage(_) => {
                            Err(api::ApiError::DuplicateMessage)
                        }
                        BlockManagerError::General(err) => {
                            Err(api::ApiError::Internal(anyhow::Error::msg(err.to_string())))
                        }
                    },
                }
            }
            Ok(false) => {
                log::debug!("Application rejected ephemera message: {:?}", api_msg);
                Err(api::ApiError::ApplicationRejectedMessage)
            }
            Err(err) => {
                log::error!("Application rejected ephemera message: {:?}", err);
                Err(api::ApiError::Application(err))
            }
        };
        reply
            .send(response)
            .expect("Error sending SubmitEphemeraMessage response to api");
        Ok(())
    }
}
