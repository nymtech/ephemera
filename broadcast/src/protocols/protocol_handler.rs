use std::marker::PhantomData;

use anyhow::Result;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::network::basic::listener::NetworkPacket;
use crate::network::BroadcastMessage;
use crate::protocols::protocol::{Kind, Protocol, ProtocolRequest};

pub struct ProtocolHandler<M, P: Protocol<M>> {
    protocol: P,
    _phantom: PhantomData<M>,
}

impl<M, P> ProtocolHandler<M, P>
where
    P: Protocol<M>,
{
    pub fn new(protocol: P) -> ProtocolHandler<M, P> {
        Self {
            protocol,
            _phantom: Default::default(),
        }
    }

    pub async fn run(
        mut self,
        mut rcv_net: Receiver<NetworkPacket<M>>,
        tx_br: Sender<BroadcastMessage<P::Response>>,
    ) {
        log::info!("Started ProtocolHandler");
        while let Some(np) = rcv_net.recv().await {
            if let Err(err) = self.process_packet(&tx_br, np).await {
                log::error!("Error processing packet: {}", err);
            }
        }
    }

    async fn process_packet(
        &mut self,
        tx_br: &Sender<BroadcastMessage<P::Response>>,
        np: NetworkPacket<M>,
    ) -> Result<()> {
        let pr_req = ProtocolRequest {
            origin_host: np.addr.to_string(),
            message: np.payload,
        };

        let pr_resp = self.protocol.handle(pr_req).await?;
        if pr_resp.kind == Kind::Broadcast {
            let bm = BroadcastMessage {
                message: pr_resp.protocol_reply,
            };
            if let Err(err) = tx_br.send(bm).await {
                log::error!("Error sending broadcast message: {}", err);
            }
        }
        Ok(())
    }
}
