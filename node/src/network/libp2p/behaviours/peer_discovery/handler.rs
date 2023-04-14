use std::task::{Context, Poll};

use libp2p::{
    core::upgrade::ReadyUpgrade,
    swarm::{
        handler::ConnectionEvent, ConnectionHandler, ConnectionHandlerEvent, KeepAlive,
        SubstreamProtocol,
    },
};
use thiserror::Error;

//Useful for versioning
pub const PROTOCOL_NAME: &[u8] = b"/ephemera/rendezvous/1.0.0";

#[derive(Error, Debug)]
pub(crate) enum RendezvousHandlerError {
    #[error("RendezvousHandlerError: {0}")]
    GeneralError(#[from] anyhow::Error),
}

pub(crate) struct Handler;

impl ConnectionHandler for Handler {
    type InEvent = ();
    type OutEvent = ();
    type Error = RendezvousHandlerError;
    type InboundProtocol = ReadyUpgrade<&'static [u8]>;
    type OutboundProtocol = ReadyUpgrade<&'static [u8]>;
    type InboundOpenInfo = ();
    type OutboundOpenInfo = ();

    fn listen_protocol(&self) -> SubstreamProtocol<ReadyUpgrade<&'static [u8]>, ()> {
        SubstreamProtocol::new(ReadyUpgrade::new(PROTOCOL_NAME), ())
    }

    fn connection_keep_alive(&self) -> KeepAlive {
        KeepAlive::No
    }

    fn poll(
        &mut self,
        _: &mut Context<'_>,
    ) -> Poll<
        ConnectionHandlerEvent<
            Self::OutboundProtocol,
            Self::OutboundOpenInfo,
            Self::OutEvent,
            Self::Error,
        >,
    > {
        Poll::Pending
    }

    fn on_behaviour_event(&mut self, _event: Self::InEvent) {}

    fn on_connection_event(
        &mut self,
        _: ConnectionEvent<
            Self::InboundProtocol,
            Self::OutboundProtocol,
            Self::InboundOpenInfo,
            Self::OutboundOpenInfo,
        >,
    ) {
    }
}
