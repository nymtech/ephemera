use tokio::sync::mpsc::{Receiver, Sender};

use crate::broadcast_protocol::broadcast::BroadcastProtocol;
use crate::broadcast_protocol::quorum::BasicQuorum;
use crate::broadcast_protocol::{BroadcastCallBack, Kind, ProtocolRequest};
use crate::network::BroadcastMessage;
use crate::request::RbMsg;
use crate::settings::Settings;

pub struct ProtocolHandler<C: BroadcastCallBack + Send> {
    broadcaster: Box<BroadcastProtocol<C>>,
}

impl<C: BroadcastCallBack + Send> ProtocolHandler<C> {
    pub fn new(settings: Settings, broadcast_callback: C) -> ProtocolHandler<C> {
        let quorum = Box::new(BasicQuorum::new(settings.quorum.clone()));
        let broadcaster = BroadcastProtocol::new(quorum, broadcast_callback, settings);
        Self {
            broadcaster: Box::new(broadcaster),
        }
    }

    pub async fn run(
        mut self,
        mut from_network: Receiver<ProtocolRequest>,
        to_network: Sender<BroadcastMessage<RbMsg>>,
    ) {
        while let Some(pr) = from_network.recv().await {
            match self.broadcaster.handle(pr).await {
                Ok(resp) => {
                    if resp.kind == Kind::Broadcast {
                        let bm = BroadcastMessage {
                            message: resp.protocol_reply,
                        };
                        if to_network.send(bm).await.is_err() {
                            panic!("to_network channel closed, aborting");
                        }
                    }
                }
                Err(err) => {
                    log::error!("Protocol error {}", err);
                }
            }
        }
    }
}
