use std::fmt::Display;
use std::time::SystemTime;

use libp2p::PeerId as Libp2pPeerId;
use serde_derive::{Deserialize, Serialize};

use crate::block::Block;
use crate::utilities;
use crate::utilities::crypto::Signature;
use crate::utilities::EphemeraId;

pub(crate) mod broadcaster;
pub(crate) mod signing;
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
pub(crate) struct RbMsg {
    ///Unique id of the message which stays the same throughout the protocol
    pub(crate) id: EphemeraId,
    ///Distinct id of the message which changes when the message is rebroadcast
    pub(crate) request_id: EphemeraId,
    ///Id of the peer that CREATED the message(not necessarily the one that sent it, with gossip it can come through a different peer)
    pub(crate) original_sender: PeerId,
    /// Inner data identifier for embedded message.
    pub(crate) data_identifier: EphemeraId,
    ///When the message was first time created
    pub(crate) timestamp: SystemTime,
    pub(crate) phase: Phase,
    pub(crate) signature: Signature,
}

impl RbMsg {
    pub(crate) fn new(data: Block, original_sender: PeerId, signature: Signature) -> RbMsg {
        RbMsg {
            id: utilities::generate_ephemera_id(),
            request_id: utilities::generate_ephemera_id(),
            original_sender,
            data_identifier: data.get_id(),
            timestamp: SystemTime::now(),
            phase: Phase::Prepare(data),
            signature,
        }
    }

    pub(crate) fn get_data(&self) -> Option<&Block> {
        match &self.phase {
            Phase::Prepare(data) => Some(data),
            Phase::Commit(data) => Some(data),
            Phase::Ack => None,
        }
    }

    pub(crate) fn reply(
        &self,
        local_id: PeerId,
        phase: Phase,
        signature: Signature,
    ) -> Self {
        RbMsg {
            id: self.id.clone(),
            request_id: utilities::generate_ephemera_id(),
            original_sender: local_id,
            data_identifier: self.data_identifier.clone(),
            timestamp: SystemTime::now(),
            phase,
            signature,
        }
    }

    pub(crate) fn prepare_reply(
        &self,
        local_id: PeerId,
        data: Block,
        signature: Signature,
    ) -> Self {
        self.reply(local_id, Phase::Prepare(data), signature)
    }

    pub(crate) fn commit_reply(
        &self,
        local_id: PeerId,
        data: Block,
        signature: Signature,
    ) -> Self {
        self.reply(local_id, Phase::Commit(data), signature)
    }

    pub(crate) fn ack_reply(&self, local_id: PeerId, signature: Signature) -> Self {
        self.reply(
            local_id,
            Phase::Ack,
            signature,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) enum Phase {
    Prepare(Block),
    Commit(Block),
    Ack,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Status {
    Pending,
    Committed,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Command {
    Broadcast,
    Drop,
}
