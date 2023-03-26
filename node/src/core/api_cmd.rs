use std::num::NonZeroUsize;

use lru::LruCache;

use crate::api::application::Application;
use crate::api::types::{ApiBlock, ApiSignature};
use crate::api::ApiCmd;
use crate::api::ApiError::ApiError;
use crate::network::libp2p::ephemera_sender::EphemeraEvent;
use crate::storage::EphemeraDatabase;
use crate::Ephemera;

pub(crate) struct ApiCmdProcessor {
    pub(crate) dht_query_cache: LruCache<
        Vec<u8>,
        tokio::sync::oneshot::Sender<Result<Option<(Vec<u8>, Vec<u8>)>, crate::api::ApiError>>,
    >,
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
        log::trace!("Processing API request: {:?}", cmd);
        match cmd {
            ApiCmd::SubmitEphemeraMessage(sm) => {
                // 1. Ask application to decide if we should accept this message.
                match ephemera.application.check_tx(sm.clone()) {
                    Ok(true) => {
                        // 2. Send to BlockManager to put into memory pool
                        let ephemera_msg: crate::block::types::message::EphemeraMessage = sm.into();
                        match ephemera
                            .block_manager
                            .new_message(ephemera_msg.clone())
                            .await
                        {
                            Ok(_) => {
                                //Gossip to network for other nodes to receive
                                ephemera
                                    .to_network
                                    .send_ephemera_event(EphemeraEvent::EphemeraMessage(
                                        ephemera_msg.into(),
                                    ))
                                    .await?;
                            }
                            Err(e) => log::error!("Error: {}", e),
                        }
                    }
                    Ok(false) => {
                        log::debug!("Application rejected ephemera message: {:?}", sm);
                    }
                    Err(err) => {
                        log::error!("Application rejected transaction: {:?}", err);
                    }
                }
            }

            ApiCmd::QueryBlockById(block_id, reply) => {
                match ephemera.storage.lock().await.get_block_by_id(block_id) {
                    Ok(Some(block)) => {
                        let api_block: ApiBlock = block.into();
                        reply
                            .send(Ok(api_block.into()))
                            .expect("Error sending block to api");
                    }
                    Ok(None) => {
                        reply.send(Ok(None)).expect("Error sending block to api");
                    }
                    Err(err) => {
                        let api_error = ApiError(format!("Error getting block by id: {err:?}"));
                        reply
                            .send(Err(api_error))
                            .expect("Error sending error to api");
                    }
                };
            }

            ApiCmd::QueryBlockByHeight(height, reply) => {
                match ephemera.storage.lock().await.get_block_by_height(height) {
                    Ok(Some(block)) => {
                        let api_block: ApiBlock = block.into();
                        reply
                            .send(Ok(api_block.into()))
                            .expect("Error sending block to api");
                    }
                    Ok(None) => {
                        reply.send(Ok(None)).expect("Error sending block to api");
                    }
                    Err(err) => {
                        let api_error = ApiError(format!("Error getting block by height: {err:?}"));
                        reply
                            .send(Err(api_error))
                            .expect("Error sending error to api");
                    }
                };
            }

            ApiCmd::QueryLastBlock(reply) => {
                match ephemera.storage.lock().await.get_last_block() {
                    Ok(Some(block)) => {
                        let api_block: ApiBlock = block.into();
                        reply
                            .send(Ok(api_block))
                            .expect("Error sending block to api");
                    }
                    Ok(None) => {
                        let api_error =
                            ApiError("Ephemera has no blocks, this is a bug".to_string());
                        reply
                            .send(Err(api_error))
                            .expect("Error sending error to api");
                    }
                    Err(err) => {
                        let api_error = ApiError(format!("Error getting last block: {err:?}",));
                        reply
                            .send(Err(api_error))
                            .expect("Error sending error to api");
                    }
                };
            }
            ApiCmd::QueryBlockSignatures(block_id, reply) => {
                match ephemera.storage.lock().await.get_block_signatures(block_id) {
                    Ok(signatures) => {
                        let signatures = signatures.map(|s| {
                            s.into_iter()
                                .map(|s| s.into())
                                .collect::<Vec<ApiSignature>>()
                        });
                        reply
                            .send(Ok(signatures))
                            .expect("Error sending block signatures to api");
                    }
                    Err(err) => {
                        let api_error =
                            ApiError(format!("Error getting block signatures: {err:?}"));
                        reply
                            .send(Err(api_error))
                            .expect("Error sending error to api");
                    }
                };
            }
            ApiCmd::QueryDht(key, reply) => {
                //FIXME: This is very loose, we could have multiple pending queries for the same key
                ephemera
                    .api_cmd_processor
                    .dht_query_cache
                    .put(key.clone(), reply);
                ephemera
                    .to_network
                    .send_ephemera_event(EphemeraEvent::QueryDht { key })
                    .await?;
            }
            ApiCmd::StoreInDht(key, value, reply) => {
                ephemera
                    .to_network
                    .send_ephemera_event(EphemeraEvent::StoreInDht { key, value })
                    .await?;
                reply.send(Ok(())).expect("Error sending reply to api");
            }
        }
        Ok(())
    }
}
