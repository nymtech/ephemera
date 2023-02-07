use std::fmt::Display;
use std::time::SystemTime;

use libp2p::PeerId as Libp2pPeerId;
use serde_derive::{Deserialize, Serialize};

use crate::utilities::id_generator;
use crate::utilities::id_generator::EphemeraId;

pub(crate) mod broadcast_callback;
pub(crate) mod broadcaster;
mod quorum;

pub(crate) type PeerIdType = Libp2pPeerId;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct PeerId(pub(crate) PeerIdType);

impl Display for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub trait BroadcastData: Clone + PartialEq + std::fmt::Debug + Send + Sync + 'static {
    fn get_id(&self) -> EphemeraId;
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct RbMsg<T: BroadcastData> {
    ///Unique id of the message which stays the same throughout the protocol
    pub id: EphemeraId,
    ///Distinct id of the message which changes when the message is rebroadcast
    pub request_id: EphemeraId,
    ///Id of the peer that CREATED the message(not necessarily the one that sent it, with gossip it can come through a different peer)
    pub original_sender: PeerId,
    /// Inner data identifier for embedded message.
    pub data_identifier: EphemeraId,
    ///When the message was first time created
    pub timestamp: SystemTime,
    pub data: Option<T>,
    pub reliable_broadcast: ReliableBroadcast,
}

//TODO create appropriate builder so that id can't be changed
impl<T: BroadcastData> RbMsg<T> {
    pub(crate) fn new(data: T, original_sender: PeerId) -> RbMsg<T> {
        RbMsg {
            id: id_generator::generate(),
            request_id: id_generator::generate(),
            original_sender,
            data_identifier: data.get_id(),
            timestamp: SystemTime::now(),
            data: Some(data),
            reliable_broadcast: ReliableBroadcast::Init,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum ReliableBroadcast {
    Init,
    Prepare,
    Commit,
    Ack,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Status {
    Pending,
    Committed,
    Finished,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Command {
    Broadcast,
    Drop,
}
