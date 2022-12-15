pub mod protocol_handler;
pub mod gossip;
pub mod quorum_consensus;

use std::error::Error;

///! Very basic abstraction for something which can be called as a protocol over handling requests and returning responses.
///!
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct ProtocolRequest<M> {
    pub origin_host: String,
    pub message: M,
}

impl<M> ProtocolRequest<M> {
    pub fn new(origin_host: String, message: M) -> ProtocolRequest<M> {
        ProtocolRequest { origin_host, message }
    }
}

#[derive(Debug, PartialEq)]
pub enum Kind {
    Broadcast,
    Drop,
}

#[derive(Debug)]
pub struct ProtocolResponse<M> {
    pub kind: Kind,
    pub protocol_reply: M,
}

#[async_trait]
pub trait Protocol<M> {
    type Response: prost::Message + Send + Sync + Default + 'static;
    type Error: Error + Send + Sync + 'static;
    async fn handle(
        &mut self,
        msg: ProtocolRequest<M>,
    ) -> Result<ProtocolResponse<Self::Response>, Self::Error>;
}
