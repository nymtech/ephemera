///! Simple gossip protocol which broadcast a message to all peers.
///! It asks the list of peers to which the message should be broadcasted from peer_discovery component
///
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::time;

use prost_types::Timestamp;
use thiserror::Error;

use crate::network::peer_discovery::PeerDiscovery;
use crate::protocols::protocol::{Kind, Protocol, ProtocolRequest, ProtocolResponse};
use crate::request::FullGossip;

pub struct FullGossipProtocol {
    peer_discovery: Box<dyn PeerDiscovery>,
    gossips: HashMap<String, GossipState>,
}

impl FullGossipProtocol {
    pub fn new(peer_discovery: Box<dyn PeerDiscovery>) -> FullGossipProtocol {
        FullGossipProtocol {
            peer_discovery,
            gossips: HashMap::new(),
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
    pub peers: HashSet<String>,
}

#[async_trait::async_trait]
impl Protocol<FullGossip, FullGossip, Vec<u8>> for FullGossipProtocol {
    type Error = FullGossipError;

    async fn handle(
        &mut self,
        msg: ProtocolRequest<FullGossip>,
    ) -> Result<ProtocolResponse<FullGossip>, Self::Error> {
        let ProtocolRequest {
            origin_host,
            message: FullGossip { id, payload, .. },
        } = msg.clone();

        log::debug!("Received message {} from peer {}", id, origin_host.clone());

        let peers: Vec<String> = match self.gossips.entry(msg.message.id.clone()) {
            Entry::Occupied(_) => vec![],
            Entry::Vacant(entry) => {
                let state = entry.insert(GossipState {
                    id: id.clone(),
                    payload: payload.clone(),
                    peers: HashSet::new(),
                });

                state.peers.insert(origin_host.clone());
                let peers = self.peer_discovery.peers_ref();
                let mut addresses = peers
                    .iter()
                    .map(|peer| peer.address.clone())
                    .collect::<Vec<String>>();
                addresses.retain(|peer| peer != &origin_host);
                addresses
            }
        };
        log::debug!("Forwarding full gossip message {} to peers {:?}", id, peers);
        let timestamp = Some(Timestamp::from(time::SystemTime::now()));
        Ok(ProtocolResponse {
            kind: Kind::BROADCAST,
            peers,
            protocol_reply: FullGossip {
                id: id.clone(),
                timestamp,
                payload: payload.clone(),
            },
        })
    }
}
