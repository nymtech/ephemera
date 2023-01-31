//! The reliable broadcast protocol, deals with signing and broadcasting messages to nodes in the network.

mod backend;
pub mod broadcast;
pub mod protocol_handler;
mod quorum;
pub(crate) mod signing;
mod test;

#[derive(Debug, Clone)]
pub struct EphemeraSigningRequest {
    pub origin: String,
    pub message: RbMsg,
}

impl EphemeraSigningRequest {
    pub fn new(origin_host: String, message: RbMsg) -> EphemeraSigningRequest {
        EphemeraSigningRequest {
            origin: origin_host,
            message,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Kind {
    Broadcast,
    Drop,
}

#[derive(Debug)]
pub struct ProtocolResponse {
    pub kind: Kind,
    pub protocol_reply: RbMsg,
}

use crate::broadcast_protocol::broadcast::ConsensusContext;

use crate::request::rb_msg::ReliableBroadcast::PrePrepare;
use crate::request::{PrePrepareMsg, RbMsg};
use anyhow::Result;
use async_trait::async_trait;
use prost_types::Timestamp;
use uuid::Uuid;

#[async_trait]
pub trait BroadcastCallBack: Send {
    async fn pre_prepare(
        &mut self,
        _msg_id: String,
        _custom_message_id: String,
        _sender: String,
        _payload: Vec<u8>,
        _ctx: &ConsensusContext,
    ) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }
    async fn prepare(
        &mut self,
        _msg_id: String,
        _custom_message_id: String,
        _sender: String,
        _payload: Vec<u8>,
        _ctx: &ConsensusContext,
    ) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }
    async fn commit(
        &mut self,
        _msg_id: String,
        _custom_message_id: String,
        _sender: String,
        _ctx: &ConsensusContext,
    ) -> Result<()> {
        Ok(())
    }
    async fn prepared(&mut self, _ctx: &ConsensusContext) -> Result<()> {
        Ok(())
    }
    async fn committed(&mut self, _ctx: &ConsensusContext) -> Result<()> {
        Ok(())
    }
}

pub fn pre_prepare_msg(sender_id: String, custom_message_id: String, payload: Vec<u8>) -> RbMsg {
    let timestamp = Timestamp::from(std::time::SystemTime::now());

    RbMsg {
        id: Uuid::new_v4().to_string(),
        node_id: sender_id,
        timestamp: Some(timestamp),
        custom_message_id,
        reliable_broadcast: Some(PrePrepare(PrePrepareMsg { payload })),
    }
}
