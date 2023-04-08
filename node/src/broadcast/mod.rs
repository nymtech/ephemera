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

//TODO: Describe/analyze how the system behaves when new block is produced while previous one is still being processed.

use std::collections::HashSet;

use serde_derive::{Deserialize, Serialize};

use crate::utilities::hash::HashType;
use crate::{
    block::types::block::Block,
    network::peer::PeerId,
    utilities::crypto::Certificate,
    utilities::id::{EphemeraId, EphemeraIdentifier},
    utilities::time::EphemeraTime,
};

pub(crate) mod bracha;
pub(crate) mod signing;

#[derive(Debug, Clone)]
pub(crate) struct ProtocolContext {
    pub(crate) local_peer_id: PeerId,
    /// Message id
    pub(crate) hash: HashType,
    /// Peers that sent prepare message(this peer included)
    pub(crate) echo: HashSet<PeerId>,
    /// Peers that sent commit message(this peer included)
    pub(crate) vote: HashSet<PeerId>,
    /// View of the network for this block
    pub(crate) topology_id: u64,
}

impl ProtocolContext {
    pub(crate) fn new(hash: HashType, local_peer_id: PeerId, topology_id: u64) -> ProtocolContext {
        ProtocolContext {
            local_peer_id,
            hash,
            echo: HashSet::new(),
            vote: HashSet::new(),
            topology_id,
        }
    }

    fn add_echo(&mut self, peer: PeerId) {
        self.echo.insert(peer);
    }

    fn add_vote(&mut self, peer: PeerId) {
        self.vote.insert(peer);
    }

    fn echoed(&self) -> bool {
        self.echo.contains(&self.local_peer_id)
    }

    fn voted(&self) -> bool {
        self.vote.contains(&self.local_peer_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct RbMsg {
    ///Unique id of the message which stays the same throughout the protocol
    pub(crate) id: EphemeraId,
    ///Distinct id of the message which changes when the message is rebroadcast
    pub(crate) request_id: EphemeraId,
    ///Id of the peer that CREATED the message(not necessarily the one that sent it, with gossip it can come through a different peer)
    pub(crate) original_sender: PeerId,
    ///When the message was created by the sender.
    pub(crate) timestamp: u64,
    ///Current phase of the protocol(Echo, Vote)
    pub(crate) phase: MessageType,
    ///Signature of the message
    pub(crate) certificate: Certificate,
}

impl RbMsg {
    pub(crate) fn new(raw: RawRbMsg, signature: Certificate) -> RbMsg {
        RbMsg {
            id: raw.id,
            request_id: raw.request_id,
            original_sender: raw.original_sender,
            timestamp: raw.timestamp,
            phase: raw.message_type,
            certificate: signature,
        }
    }

    pub(crate) fn get_block(&self) -> &Block {
        match &self.phase {
            MessageType::Echo(block) | MessageType::Vote(block) => block,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct RawRbMsg {
    pub(crate) id: EphemeraId,
    pub(crate) request_id: EphemeraId,
    pub(crate) original_sender: PeerId,
    pub(crate) timestamp: u64,
    pub(crate) message_type: MessageType,
}

impl RawRbMsg {
    pub(crate) fn new(block: Block, original_sender: PeerId) -> RawRbMsg {
        RawRbMsg {
            id: EphemeraId::generate(),
            request_id: EphemeraId::generate(),
            original_sender,
            timestamp: EphemeraTime::now(),
            message_type: MessageType::Echo(block),
        }
    }

    pub(crate) fn block_ref(&self) -> &Block {
        match &self.message_type {
            MessageType::Echo(block) | MessageType::Vote(block) => block,
        }
    }

    pub(crate) fn block(&self) -> Block {
        match &self.message_type {
            MessageType::Echo(block) | MessageType::Vote(block) => block.clone(),
        }
    }

    pub(crate) fn reply(&self, local_id: PeerId, phase: MessageType) -> Self {
        RawRbMsg {
            id: self.id.clone(),
            request_id: EphemeraId::generate(),
            original_sender: local_id,
            timestamp: EphemeraTime::now(),
            message_type: phase,
        }
    }

    pub(crate) fn echo_reply(&self, local_id: PeerId, data: Block) -> Self {
        self.reply(local_id, MessageType::Echo(data))
    }

    pub(crate) fn vote_reply(&self, local_id: PeerId, data: Block) -> Self {
        self.reply(local_id, MessageType::Vote(data))
    }
}

impl From<RbMsg> for RawRbMsg {
    fn from(msg: RbMsg) -> Self {
        RawRbMsg {
            id: msg.id,
            request_id: msg.request_id,
            original_sender: msg.original_sender,
            timestamp: msg.timestamp,
            message_type: msg.phase,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) enum MessageType {
    Echo(Block),
    Vote(Block),
}

#[derive(Debug, PartialEq)]
pub(crate) enum Status {
    Pending,
    Completed,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Command {
    Broadcast,
    Drop,
}
