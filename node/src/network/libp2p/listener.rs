use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;
use tokio::sync::mpsc;

use crate::block::{Block, SignedMessage};
use crate::broadcast::RbMsg;
use crate::network::libp2p::swarm::NetworkMessage;

pub struct NetworkMessages;

impl NetworkMessages {
    pub(crate) fn init() -> (
        NetworkBroadcastReceiver,
        BroadcastReceiverHandle,
        MessagesReceiver,
        MessageReceiverHandle,
    ) {
        let (broadcast_listener, broadcast_listener_handle) = NetworkBroadcastReceiver::new();
        let (message_listener, message_listener_handle) = MessagesReceiver::new();
        (
            broadcast_listener,
            broadcast_listener_handle,
            message_listener,
            message_listener_handle,
        )
    }
}

#[derive(Clone)]
pub struct BroadcastReceiverHandle {
    pub(crate) rb_msg_from_net_rcv: mpsc::Sender<NetworkMessage<RbMsg<Block>>>,
}

impl BroadcastReceiverHandle {
    pub(crate) async fn send(
        &mut self,
        msg: NetworkMessage<RbMsg<Block>>,
    ) -> Result<(), mpsc::error::SendError<NetworkMessage<RbMsg<Block>>>> {
        self.rb_msg_from_net_rcv.send(msg).await
    }
}

pub(crate) struct NetworkBroadcastReceiver {
    pub(crate) rb_msg_from_net_rcv: mpsc::Receiver<NetworkMessage<RbMsg<Block>>>,
}

impl NetworkBroadcastReceiver {
    pub fn new() -> (NetworkBroadcastReceiver, BroadcastReceiverHandle) {
        let (broadcast_messages_from_net_tx, broadcast_messages_from_net_rcv) = mpsc::channel(100);
        let handle = BroadcastReceiverHandle {
            rb_msg_from_net_rcv: broadcast_messages_from_net_tx,
        };
        let listener = NetworkBroadcastReceiver {
            rb_msg_from_net_rcv: broadcast_messages_from_net_rcv,
        };
        (listener, handle)
    }
}

impl Stream for NetworkBroadcastReceiver {
    type Item = NetworkMessage<RbMsg<Block>>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if let Ok(msg) = this.rb_msg_from_net_rcv.try_recv() {
            return Poll::Ready(Some(msg));
        }
        Poll::Ready(None)
    }
}

#[derive(Clone)]
pub(crate) struct MessageReceiverHandle {
    pub(crate) signed_messages_from_net_tx: mpsc::Sender<SignedMessage>,
}

impl MessageReceiverHandle {
    pub async fn send(
        &mut self,
        msg: SignedMessage,
    ) -> Result<(), mpsc::error::SendError<SignedMessage>> {
        self.signed_messages_from_net_tx.send(msg).await
    }
}

pub(crate) struct MessagesReceiver {
    pub(crate) signed_messages_from_net_rcv: mpsc::Receiver<SignedMessage>,
}

impl MessagesReceiver {
    pub fn new() -> (MessagesReceiver, MessageReceiverHandle) {
        let (signed_messages_from_net_tx, signed_messages_from_net_rcv) = mpsc::channel(100);
        let handle = MessageReceiverHandle {
            signed_messages_from_net_tx,
        };
        let listener = MessagesReceiver {
            signed_messages_from_net_rcv,
        };

        (listener, handle)
    }
}
