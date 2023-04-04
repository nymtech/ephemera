use ephemera::crypto::Keypair;
use ephemera::ephemera_api::{ApiEphemeraMessage, RawApiEphemeraMessage};

pub(crate) fn create_ephemera_message(
    label: String,
    data: Vec<u8>,
    key_pair: &Keypair,
) -> ApiEphemeraMessage {
    let message = RawApiEphemeraMessage::new(label, data);
    message.sign(&key_pair).expect("Failed to sign message")
}
