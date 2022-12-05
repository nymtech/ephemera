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
    BROADCAST,
}

#[derive(Debug)]
pub struct ProtocolResponse<M> {
    pub kind: Kind,
    pub peers: Vec<String>,
    pub protocol_reply: M,
}

#[async_trait]
pub trait Protocol<Req, Res, Body> {
    type Error;
    async fn handle(&mut self, msg: ProtocolRequest<Req>) -> Result<ProtocolResponse<Res>, Self::Error>;
}
