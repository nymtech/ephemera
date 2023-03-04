//! This a basic implementation of a broadcast_protocol where participating peers go through three rounds to
//! reach a consensus on if/when to deliver a message.
//!
//! PRE-PREPARE:
//!      1. Upon receiving pre-prepare message, peer sends PREPARE message to all other peers.
//!
//! PREPARE:
//!     1. When peer receives a PREPARE message, it sends PREPARE message to all other peers.
//! COMMIT:
//!     1. When peer receives a quorum of PREPARE messages, it sends COMMIT message to all peers.
//!     2. When peer receives quorum of COMMIT messages, it considers the message as committed and can deliver it.
//!
//! It is possible to register a callback which will be called for the following events:
//!     1. Pre-prepare message received
//!     2. Prepare message received
//!     3. Prepare quorum reached
//!     4. Commit message received
//!     5. Commit quorum reached
//!
//! Callback can return optional (modified) message payload which is sent along with consensus message
//!  instead of the original payload from client.
//!

//! Limitations:
//! - Prepare and commit messages can reach out of order due to network and node processing delays. Nevertheless,
//!   a peer won't commit a message until it receives a quorum of prepare messages.
//! - Current implementation makes only progress(updates its state machine) when it receives a message from another peer.
//!   If for some reason messages are lost, the broadcast_protocol will not make progress. This can be fixed by introducing a timer and some concept
//!   of views/epoch.
//! - It doesn't try to total order different messages. All messages reach quorum consensus independently.
//!   All it does is that a quorum or no quorum of peers deliver the message.
//! - It doesn't verify the other peers authenticity.
//!   Also this can be a task for an upstream layer(gossip...) which handles networking and peers relationship.

use crate::block::types::block::Block;
use serde_derive::{Deserialize, Serialize};

use crate::utilities;
use crate::utilities::crypto::{PeerId, Signature};
use crate::utilities::EphemeraId;

pub(crate) mod broadcaster;
mod quorum;
pub(crate) mod signing;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct RbMsg {
    ///Unique id of the message which stays the same throughout the protocol
    pub(crate) id: EphemeraId,
    ///Distinct id of the message which changes when the message is rebroadcast
    pub(crate) request_id: EphemeraId,
    ///Id of the peer that CREATED the message(not necessarily the one that sent it, with gossip it can come through a different peer)
    pub(crate) original_sender: PeerId,
    /// Inner data identifier for embedded message.(FIXME: this is sort of hack)
    pub(crate) data_identifier: EphemeraId,
    ///When the message was created by the sender.
    pub(crate) timestamp: u64,
    ///Current phase of the protocol(Prepare, Commit, Ack)
    pub(crate) phase: Phase,
    ///Signature of the message. The same as block signature.
    pub(crate) signature: Signature,
}

impl RbMsg {
    pub(crate) fn new(data: Block, original_sender: PeerId, signature: Signature) -> RbMsg {
        RbMsg {
            id: utilities::generate_ephemera_id(),
            request_id: utilities::generate_ephemera_id(),
            original_sender,
            data_identifier: data.header.id.clone(),
            timestamp: utilities::time::ephemera_now(),
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

    pub(crate) fn reply(&self, local_id: PeerId, phase: Phase, signature: Signature) -> Self {
        RbMsg {
            id: self.id.clone(),
            request_id: utilities::generate_ephemera_id(),
            original_sender: local_id,
            data_identifier: self.data_identifier.clone(),
            timestamp: utilities::time::ephemera_now(),
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

    pub(crate) fn commit_reply(&self, local_id: PeerId, data: Block, signature: Signature) -> Self {
        self.reply(local_id, Phase::Commit(data), signature)
    }

    pub(crate) fn ack_reply(&self, local_id: PeerId, signature: Signature) -> Self {
        self.reply(local_id, Phase::Ack, signature)
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
