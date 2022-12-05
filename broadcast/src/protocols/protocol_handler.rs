///! Receives messages from network and passes them to protocol.
///! Supports encoding and decoding of messages for a specific protocol.
///!
///! Also allows protocol to broadcast messages by forwarding broadcast messages to a broadcaster.
///!

use std::{error, io};

use anyhow::Result;
use bytes::BytesMut;
use prost::Message;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_util::codec::Encoder;

use crate::network::broadcaster::BroadcastMessage;
use crate::network::codec::ProtoCodec;
use crate::network::listener::NetworkPacket;
use crate::protocols::protocol::{Kind, Protocol, ProtocolRequest, ProtocolResponse};

type ProtocolBox<Req, Res, Msg, E> = Box<dyn Protocol<Req, Res, Msg, Error = E> + Send>;

pub struct ProtocolHandler<Req, Res, Msg, E> {
    protocol: ProtocolBox<Req, Res, Msg, E>,
    codec: ProtoCodec<Req, Res>,
}

impl<Req, Res, Msg, E> ProtocolHandler<Req, Res, Msg, E>
where
    Req: Message + Default + Clone,
    Res: Message,
    E: error::Error + Send + Sync + 'static,
{
    pub fn new(
        protocol: Box<dyn Protocol<Req, Res, Msg, Error = E> + Send>,
        codec: ProtoCodec<Req, Res>,
    ) -> ProtocolHandler<Req, Res, Msg, E> {
        Self { protocol, codec }
    }

    pub async fn run(mut self, mut rcv_net: Receiver<NetworkPacket<Req>>, tx_br: Sender<BroadcastMessage>) {
        while let Some(np) = rcv_net.recv().await {
            if let Err(err) = self.process_packet(&tx_br, np).await {
                log::error!("Error processing packet: {}", err);
            }
        }
    }

    async fn process_packet(
        &mut self,
        tx_br: &Sender<BroadcastMessage>,
        np: NetworkPacket<Req>,
    ) -> Result<()> {
        let pr_req = ProtocolRequest {
            origin_host: np.addr,
            message: np.payload,
        };

        let pr_resp = self.protocol.handle(pr_req).await?;
        if pr_resp.kind == Kind::BROADCAST {
            if let Err(e) = self.broadcast(&tx_br, pr_resp).await {
                log::error!("Error broadcasting message: {}", e);
            }
        }
        Ok(())
    }

    async fn broadcast(
        &mut self,
        tx_br: &Sender<BroadcastMessage>,
        pro_res: ProtocolResponse<Res>,
    ) -> Result<(), String> {
        let encoded = self
            .encode_packet(pro_res.protocol_reply)
            .map_err(|e| format!("Error encoding packet: {}", e))?;

        let br_msg = BroadcastMessage {
            peers: pro_res.peers,
            payload: encoded.clone(),
        };
        if let Err(err) = tx_br.send(br_msg).await {
            log::error!("Error sending broadcast message: {}", err);
        }
        Ok(())
    }

    fn encode_packet(&mut self, packet: Res) -> Result<Vec<u8>, io::Error> {
        let mut buf = BytesMut::new();
        self.codec.encode(packet, &mut buf)?;
        Ok(buf.to_vec())
    }
}
