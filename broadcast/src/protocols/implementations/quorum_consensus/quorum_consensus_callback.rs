///! Provides hooks into the consensus protocol.
///! An implementation can be used to provide custom logic for the consensus protocol in terms of payload
///! and stopping processing by returning errors.
///!
use anyhow::Result;

use crate::protocols::implementations::quorum_consensus::quorum_consensus::ConsensusContext;

pub trait QuorumConsensusCallBack<Req, Res, Body>: Send {
    fn pre_prepare(
        &mut self,
        _msg_id: String,
        _sender: String,
        _payload: Vec<u8>,
        _ctx: &ConsensusContext,
    ) -> Result<Option<Body>> {
        Ok(None)
    }
    fn prepare(
        &mut self,
        _msg_id: String,
        _sender: String,
        _payload: Vec<u8>,
        _ctx: &ConsensusContext,
    ) -> Result<Option<Body>> {
        Ok(None)
    }
    fn commit(&mut self, _msg_id: String, _sender: String, _ctx: &ConsensusContext) -> Result<()> {
        Ok(())
    }
    fn prepared(&self, _ctx: &ConsensusContext) -> Result<()> {
        Ok(())
    }
    fn committed(&self, _ctx: &ConsensusContext) -> Result<()> {
        Ok(())
    }
}
