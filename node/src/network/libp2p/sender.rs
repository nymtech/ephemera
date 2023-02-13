use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time;
use tokio::time::Interval;

use crate::block::SignedMessage;
use crate::broadcast::RbMsg;

impl EphemeraMessagesReceiver {
    pub(crate) fn init() -> (EphemeraMessagesNotifier, EphemeraMessagesReceiver) {
        let (message_sender_tx, message_sender_rcv) = mpsc::channel(100);
        let (broadcast_sender_tx, broadcast_sender_rcv) = mpsc::channel(100);

        let receiver = EphemeraMessagesReceiver {
            message_sender_rcv,
            broadcast_sender_rcv,
        };
        let sender = EphemeraMessagesNotifier::new(message_sender_tx, broadcast_sender_tx);
        (sender, receiver)
    }
}

pub struct EphemeraMessagesReceiver {
    pub(crate) message_sender_rcv: mpsc::Receiver<SignedMessage>,
    pub(crate) broadcast_sender_rcv: mpsc::Receiver<RbMsg>,
}

pub(crate) struct EphemeraMessagesNotifier {
    pub(crate) pending_messages: Vec<SignedMessage>,
    pub(crate) pending_broadcasts: Vec<RbMsg>,
    pub(crate) message_sender_tx: mpsc::Sender<SignedMessage>,
    pub(crate) broadcast_sender_tx: mpsc::Sender<RbMsg>,
    pub(crate) flush_timer: Interval,
}

impl EphemeraMessagesNotifier {
    pub(crate) fn new(
        message_sender_tx: mpsc::Sender<SignedMessage>,
        broadcast_sender_tx: mpsc::Sender<RbMsg>,
    ) -> Self {
        Self {
            pending_messages: Default::default(),
            pending_broadcasts: Default::default(),
            message_sender_tx,
            broadcast_sender_tx,
            flush_timer: time::interval(Duration::from_millis(100)),
        }
    }

    pub fn gossip_signed_message(&mut self, msg: SignedMessage) {
        log::trace!("Gossiping signed message: {:?}", msg);
        self.pending_messages.push(msg);
    }

    pub async fn send_protocol_message(&mut self, msg: RbMsg) {
        log::trace!("Sending broadcaster message: {:?}", msg);
        self.pending_broadcasts.push(msg);
        self.flush().await.unwrap();
    }

    pub(crate) async fn flush(&mut self) -> anyhow::Result<()> {
        for msg in self.pending_messages.drain(..) {
            log::trace!("Sending message: {:?}", msg);
            self.message_sender_tx.try_send(msg)?;
        }
        for msg in self.pending_broadcasts.drain(..) {
            log::trace!("Sending broadcast: {:?}", msg);
            self.broadcast_sender_tx.send(msg).await?;
        }
        Ok(())
    }
}
