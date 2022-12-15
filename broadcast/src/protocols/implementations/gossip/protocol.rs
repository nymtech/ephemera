use std::num::NonZeroUsize;
use std::time;

use lru::LruCache;
use prost_types::Timestamp;
use thiserror::Error;

use crate::protocols::protocol::{Kind, Protocol, ProtocolRequest, ProtocolResponse};
use crate::request::FullGossip;

pub struct FullGossipProtocol {
    gossips: LruCache<String, GossipState>,
}

impl FullGossipProtocol {
    pub fn new() -> FullGossipProtocol {
        FullGossipProtocol {
            gossips: LruCache::new(NonZeroUsize::new(1000).unwrap()),
        }
    }
}

#[derive(Error, Debug)]
pub enum FullGossipError {
    #[error("Gossip Error")]
    GossipError,
}

pub struct GossipState {
    pub id: String,
    pub payload: Vec<u8>,
}

#[async_trait::async_trait]
impl Protocol<FullGossip> for FullGossipProtocol {
    type Response = FullGossip;
    type Error = FullGossipError;

    async fn handle(
        &mut self,
        msg: ProtocolRequest<FullGossip>,
    ) -> Result<ProtocolResponse<FullGossip>, Self::Error> {
        let ProtocolRequest {
            origin_host,
            message: FullGossip { id, payload, .. },
        } = msg;

        log::debug!("Received message {} from peer {}", id, origin_host);

        if self.gossips.get(&id).is_none() {
            self.gossips.put(
                id.clone(),
                GossipState {
                    id: id.clone(),
                    payload: payload.clone(),
                },
            );

            log::debug!("Forwarding full gossip message {}", id);

            let timestamp = Some(Timestamp::from(time::SystemTime::now()));
            Ok(ProtocolResponse {
                kind: Kind::Broadcast,
                protocol_reply: FullGossip {
                    id: id.clone(),
                    timestamp,
                    payload,
                },
            })
        } else {
            Ok(ProtocolResponse {
                kind: Kind::Drop,
                protocol_reply: FullGossip {
                    id: id.clone(),
                    timestamp: None,
                    payload: vec![],
                },
            })
        }
    }
}
