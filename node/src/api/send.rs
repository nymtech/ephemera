
use crate::api::EphemeraSigningRequest;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct MessageSendApi {
    to_protocol: mpsc::Sender<EphemeraSigningRequest>,
}

impl MessageSendApi {
    pub fn new(ephemera_incoming_mgs: mpsc::Sender<EphemeraSigningRequest>) -> Self {
        Self {
            to_protocol: ephemera_incoming_mgs,
        }
    }

    pub async fn send_message(&mut self, msg: EphemeraSigningRequest) {
        if let Err(err) = self.to_protocol.send(msg).await {
            panic!("Receiver closed: {}, unable to continue", err);
        }
    }
}
