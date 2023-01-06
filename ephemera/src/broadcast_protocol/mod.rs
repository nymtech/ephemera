pub mod broadcast;
pub mod protocol_handler;
pub mod quorum;
mod test;

#[derive(Debug, Clone)]
pub struct ProtocolRequest {
    pub origin: String,
    pub message: RbMsg,
}

impl ProtocolRequest {
    pub fn new(origin_host: String, message: RbMsg) -> ProtocolRequest {
        ProtocolRequest {
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

use crate::request::RbMsg;
use anyhow::Result;

pub trait BroadcastCallBack: Send {
    fn pre_prepare(
        &mut self,
        _msg_id: String,
        _sender: String,
        _payload: Vec<u8>,
        _ctx: &ConsensusContext,
    ) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }
    fn prepare(
        &mut self,
        _msg_id: String,
        _sender: String,
        _payload: Vec<u8>,
        _ctx: &ConsensusContext,
    ) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }
    fn commit(&mut self, _msg_id: String, _sender: String, _ctx: &ConsensusContext) -> Result<()> {
        Ok(())
    }
    fn prepared(&mut self, _ctx: &ConsensusContext) -> Result<()> {
        Ok(())
    }
    fn committed(&mut self, _ctx: &ConsensusContext) -> Result<()> {
        Ok(())
    }
}
