use clap::Parser;

use crate::utilities::crypto::{EphemeraKeypair, KeyPair};
use crate::utilities::crypto::signer::CryptoApi;

#[derive(Debug, Clone, Parser)]
pub struct SignMessageCmd {
    #[clap(short, long)]
    request_id: String,
    #[clap(short, long)]
    data: String,
    #[clap(short, long)]
    private_key: String,
}

impl SignMessageCmd {
    pub async fn execute(self) {
        let signature_hex =
            CryptoApi::sign_message(self.request_id, self.data, self.private_key).unwrap();
        println!("Signature in hex: {:>5}", signature_hex.signature);
    }
}

#[derive(Debug, Clone, Parser)]
pub struct GenerateKeypairCmd;

impl GenerateKeypairCmd {
    pub async fn execute(self) {
        let keypair = EphemeraKeypair::generate().expect("Invalid keypair");
        let keypair_hex = keypair.format_hex().expect("Invalid keypair");
        println!("Private key: {:>5}", keypair_hex.private_key);
        println!("Public  key: {:>5}", keypair_hex.public_key);
    }
}
