use std::num::NonZeroUsize;

use lru::LruCache;

use crate::{
    api::{
        self,
        application::Application,
        types::{ApiBlock, ApiCertificate},
        ApiCmd,
    },
    block::manager::BlockManagerError,
    network::libp2p::ephemera_sender::EphemeraEvent,
    Ephemera,
};

pub(crate) struct ApiCmdProcessor {
    pub(crate) dht_query_cache: LruCache<
        Vec<u8>,
        tokio::sync::oneshot::Sender<Result<Option<(Vec<u8>, Vec<u8>)>, api::ApiError>>,
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
            ApiCmd::SubmitEphemeraMessage(sm, reply) => {
                // 1. Ask application to decide if we should accept this message.
                match ephemera.application.check_tx(*sm.clone()) {
                    Ok(true) => {
                        log::trace!("Application accepted ephemera message: {:?}", sm);
                        // 2. Send to BlockManager to put into memory pool
                        let ephemera_msg: crate::block::types::message::EphemeraMessage =
                            (*sm).try_into()?;
                        match ephemera
                            .block_manager
                            .on_new_message(ephemera_msg.clone())
                            .await
                        {
                            Ok(_) => {
                                //Gossip to network for other nodes to receive
                                match ephemera
                                    .to_network
                                    .send_ephemera_event(EphemeraEvent::EphemeraMessage(
                                        ephemera_msg.into(),
                                    ))
                                    .await
                                {
                                    Ok(_) => {
                                        reply.send(Ok(())).expect(
                                            "Error sending SubmitEphemeraMessage response to api",
                                        );
                                    }
                                    Err(err) => {
                                        log::error!("Error: {}", err);
                                        reply.send(Err(api::ApiError::Internal(err))).expect(
                                            "Error sending SubmitEphemeraMessage response to api",
                                        );
                                    }
                                }
                            }
                            Err(err) => {
                                log::error!("Error: {}", err);
                                match err {
                                    BlockManagerError::DuplicateMessage(_) => {
                                        reply.send(Err(api::ApiError::DuplicateMessage)).expect(
                                            "Error sending SubmitEphemeraMessage response to api",
                                        );
                                    }
                                    BlockManagerError::Internal(_) => {
                                        reply.send(Err(api::ApiError::Internal(
                                            anyhow::Error::msg(err.to_string()),
                                        )))
                                            .expect("Error sending SubmitEphemeraMessage response to api");
                                    }
                                }
                            }
                        }
                    }
                    Ok(false) => {
                        log::debug!("Application rejected ephemera message: {:?}", sm);
                        reply
                            .send(Err(api::ApiError::ApplicationRejectedMessage))
                            .expect("Error sending SubmitEphemeraMessage response to api");
                    }
                    Err(err) => {
                        log::error!("Application rejected transaction: {:?}", err);
                        reply
                            .send(Err(api::ApiError::Internal(err)))
                            .expect("Error sending SubmitEphemeraMessage response to api");
                    }
                }
            }

            ApiCmd::QueryBlockById(block_id, reply) => {
                match ephemera.storage.lock().await.get_block_by_id(block_id) {
                    Ok(Some(block)) => {
                        let api_block: ApiBlock = block.into();
                        reply
                            .send(Ok(api_block.into()))
                            .expect("Error sending QueryBlockById response to api");
                    }
                    Ok(None) => {
                        reply
                            .send(Ok(None))
                            .expect("Error sending QueryBlockById response to api");
                    }
                    Err(err) => {
                        reply
                            .send(Err(api::ApiError::Internal(err)))
                            .expect("Error sending QueryBlockById response to api");
                    }
                };
            }

            ApiCmd::QueryBlockByHeight(height, reply) => {
                match ephemera.storage.lock().await.get_block_by_height(height) {
                    Ok(Some(block)) => {
                        let api_block: ApiBlock = block.into();
                        reply
                            .send(Ok(api_block.into()))
                            .expect("Error sending QueryBlockByHeight response to api");
                    }
                    Ok(None) => {
                        reply
                            .send(Ok(None))
                            .expect("Error sending QueryBlockByHeight response to api");
                    }
                    Err(err) => {
                        reply
                            .send(Err(api::ApiError::Internal(err)))
                            .expect("Error sending QueryBlockByHeight response to api");
                    }
                };
            }

            ApiCmd::QueryLastBlock(reply) => {
                match ephemera.storage.lock().await.get_last_block() {
                    Ok(Some(block)) => {
                        reply
                            .send(Ok(block.into()))
                            .expect("Error sending QueryLastBlock response to api");
                    }
                    Ok(None) => {
                        reply
                            .send(Err(api::ApiError::Internal(anyhow::Error::msg(
                                "No blocks found, this is a bug!",
                            ))))
                            .expect("Error sending QueryLastBlock response to api");
                    }
                    Err(err) => {
                        reply
                            .send(Err(api::ApiError::Internal(err)))
                            .expect("Error sending QueryLastBlock response to api");
                    }
                };
            }
            ApiCmd::QueryBlockSignatures(block_id, reply) => {
                match ephemera
                    .storage
                    .lock()
                    .await
                    .get_block_certificates(block_id)
                {
                    Ok(signatures) => {
                        let signatures = signatures.map(|s| {
                            s.into_iter()
                                .map(|s| s.into())
                                .collect::<Vec<ApiCertificate>>()
                        });
                        reply
                            .send(Ok(signatures))
                            .expect("Error sending QueryBlockSignatures response to api");
                    }
                    Err(err) => {
                        reply
                            .send(Err(api::ApiError::Internal(err)))
                            .expect("Error sending QueryBlockSignatures response to api");
                    }
                };
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
                    }
                    Err(err) => {
                        log::error!("Error sending QueryDht to network: {:?}", err);

                        reply
                            .send(Err(api::ApiError::Internal(err)))
                            .expect("Error sending QueryDht response to api");
                    }
                }
            }
            ApiCmd::StoreInDht(key, value, reply) => {
                if let Err(err) = ephemera
                    .to_network
                    .send_ephemera_event(EphemeraEvent::StoreInDht { key, value })
                    .await
                {
                    log::error!("Error sending StoreInDht to network: {:?}", err);

                    reply
                        .send(Err(api::ApiError::Internal(err)))
                        .expect("Error sending StoreInDht response to api");
                } else {
                    reply
                        .send(Ok(()))
                        .expect("Error sending StoreInDht response to api");
                }
            }
        }
        Ok(())
    }
}
