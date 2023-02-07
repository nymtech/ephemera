use anyhow::Result;
use async_trait::async_trait;

use crate::block::Block;
use crate::broadcast::broadcaster::ConsensusContext;
use crate::broadcast::PeerId;

#[async_trait]
//TODO: crate private for now
pub(crate) trait BroadcastCallBack: Send {
    type Message: Clone + Send + Sync + 'static;

    async fn pre_prepare(
        &mut self,
        _message: &Self::Message,
        ctx: &ConsensusContext,
    ) -> Result<Option<Self::Message>> {
        log::trace!("PRE_PREPARE {:?}", ctx);
        Ok(None)
    }
    async fn prepare(
        &mut self,
        _origin: PeerId,
        _message: &Self::Message,
        ctx: &ConsensusContext,
    ) -> Result<Option<Self::Message>> {
        log::trace!("PREPARE {:?}", ctx);
        Ok(None)
    }

    async fn commit(&mut self, _origin: &PeerId, ctx: &ConsensusContext) -> Result<()> {
        log::trace!("COMMIT {:?}", ctx);
        Ok(())
    }
    async fn prepared(&mut self, ctx: &ConsensusContext) -> Result<()> {
        log::trace!("PREPARED {:?}", ctx);
        Ok(())
    }
    async fn committed(&mut self, ctx: &ConsensusContext) -> Result<()> {
        log::trace!("COMMITTED {:?}", ctx);
        Ok(())
    }
}

pub(crate) struct DummyBroadcastCallBack;

#[async_trait]
impl BroadcastCallBack for DummyBroadcastCallBack {
    type Message = Block;
}
