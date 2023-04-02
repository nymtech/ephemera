use clap::Parser;

use crate::utilities::crypto::EphemeraPublicKey;
use crate::utilities::{Ed25519Keypair, EphemeraKeypair};

#[derive(Debug, Clone, Parser)]
pub struct GenerateKeypairCmd;

impl GenerateKeypairCmd {
    pub async fn execute(self) {
        let keypair = Ed25519Keypair::generate(None);
        println!("Keypair: {:>5}", keypair.to_base58());
        println!("Public key: {:>5}", keypair.public_key().to_base58());
    }
}
