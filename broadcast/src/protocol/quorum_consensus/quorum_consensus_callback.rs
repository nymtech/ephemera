///! Provides hooks into the consensus protocol.
///! An implementation can be used to provide custom logic for the consensus protocol in terms of payload
///! and stopping processing by returning errors.
///!
use anyhow::Result;
use crate::protocol::quorum_consensus::protocol::ConsensusContext;

pub trait QuorumConsensusCallBack<Req, Res>: Send {
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
