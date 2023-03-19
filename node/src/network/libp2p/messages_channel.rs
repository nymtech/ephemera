use tokio::sync::mpsc;

use crate::block::types::message::EphemeraMessage;
use crate::broadcast::RbMsg;

pub(crate) struct EphemeraNetworkCommunication;

impl EphemeraNetworkCommunication {
    pub(crate) fn init() -> (NetCommunicationSender, NetCommunicationReceiver) {
        let (message_tx, message_rcv) = mpsc::channel(1000);
        let (protocol_tx, protocol_rcv) = mpsc::channel(1000);

        let receiver = NetCommunicationReceiver::new(message_rcv, protocol_rcv);
        let sender = NetCommunicationSender::new(message_tx, protocol_tx);

        (sender, receiver)
    }
}

//Receives messages from the network
pub(crate) struct NetCommunicationReceiver {
    pub(crate) ephemera_message_receiver: mpsc::Receiver<EphemeraMessage>,
    pub(crate) protocol_msg_receiver: mpsc::Receiver<RbMsg>,
}

impl NetCommunicationReceiver {
    pub(crate) fn new(
        ephemera_message_receiver: mpsc::Receiver<EphemeraMessage>,
        protocol_msg_receiver: mpsc::Receiver<RbMsg>,
    ) -> Self {
        Self {
            ephemera_message_receiver,
            protocol_msg_receiver,
        }
    }
}

//Sends messages to the network
pub(crate) struct NetCommunicationSender {
    pub(crate) message_sender_tx: mpsc::Sender<EphemeraMessage>,
    pub(crate) broadcast_sender_tx: mpsc::Sender<RbMsg>,
}

impl NetCommunicationSender {
    pub(crate) fn new(
        message_sender_tx: mpsc::Sender<EphemeraMessage>,
        broadcast_sender_tx: mpsc::Sender<RbMsg>,
    ) -> Self {
        Self {
            message_sender_tx,
            broadcast_sender_tx,
        }
    }

    pub async fn send_ephemera_message_raw(&mut self, msg: Vec<u8>) -> anyhow::Result<()> {
        let msg = serde_json::from_slice(&msg[..])?;
        self.send_ephemera_message(msg).await
    }

    pub async fn send_protocol_message_raw(&mut self, msg: Vec<u8>) -> anyhow::Result<()> {
        let msg: RbMsg = serde_json::from_slice(&msg[..])?;
        self.send_protocol_message(msg).await
    }

    pub async fn send_ephemera_message(&mut self, msg: EphemeraMessage) -> anyhow::Result<()> {
        log::trace!("Gossiping ephemera message: {:?}", msg);
        self.message_sender_tx
            .send(msg)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    pub async fn send_protocol_message(&mut self, msg: RbMsg) -> anyhow::Result<()> {
        log::trace!("Received protocol message: {:?}", msg);
        self.broadcast_sender_tx
            .send(msg)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }
}
