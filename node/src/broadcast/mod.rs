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

use std::collections::HashSet;

use serde_derive::{Deserialize, Serialize};

use crate::block::types::block::Block;
use crate::utilities;
use crate::utilities::crypto::{PeerId, Signature};
use crate::utilities::EphemeraId;

pub(crate) mod bracha;
pub(crate) mod signing;

pub(crate) trait Quorum {
    fn check_threshold(&self, ctx: &ConsensusContext, phase: MessageType) -> bool;
}

#[derive(Debug, Default, Clone)]
pub(crate) struct ConsensusContext {
    /// Message id
    pub(crate) id: EphemeraId,
    /// Peers that sent prepare message(this peer included)
    pub(crate) echo: HashSet<PeerId>,
    /// Peers that sent commit message(this peer included)
    pub(crate) vote: HashSet<PeerId>,
    /// Flag indicating if the message has received enough prepare messages
    pub(crate) echo_threshold: bool,
    /// Flag indicating if the message has received enough commit messages
    pub(crate) vote_threshold: bool,
}

impl ConsensusContext {
    pub(crate) fn new(id: EphemeraId) -> ConsensusContext {
        ConsensusContext {
            id,
            echo: HashSet::new(),
            vote: HashSet::new(),
            echo_threshold: false,
            vote_threshold: false,
        }
    }

    fn add_echo(&mut self, peer: PeerId) {
        self.echo.insert(peer);
    }

    fn add_vote(&mut self, peer: PeerId) {
        self.vote.insert(peer);
    }
}

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
    ///Current phase of the protocol(Echo, Vote)
    pub(crate) phase: MessageType,
    ///Signature of the message. The same as block signature.
    pub(crate) signature: Signature,
}

impl RbMsg {
    pub(crate) fn new(block: Block, original_sender: PeerId, signature: Signature) -> RbMsg {
        RbMsg {
            id: utilities::generate_ephemera_id(),
            request_id: utilities::generate_ephemera_id(),
            original_sender,
            data_identifier: block.header.id.clone(),
            timestamp: utilities::time::ephemera_now(),
            phase: MessageType::Echo(block),
            signature,
        }
    }

    pub(crate) fn get_block(&self) -> &Block {
        match &self.phase {
            MessageType::Echo(block) | MessageType::Vote(block) => block,
        }
    }

    pub(crate) fn reply(&self, local_id: PeerId, phase: MessageType, signature: Signature) -> Self {
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

    pub(crate) fn echo_reply(&self, local_id: PeerId, data: Block, signature: Signature) -> Self {
        self.reply(local_id, MessageType::Echo(data), signature)
    }

    pub(crate) fn vote_reply(&self, local_id: PeerId, data: Block, signature: Signature) -> Self {
        self.reply(local_id, MessageType::Vote(data), signature)
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) enum MessageType {
    Echo(Block),
    Vote(Block),
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
