use clap::Parser;

use crate::utilities::crypto::EphemeraPublicKey;
use crate::utilities::{to_hex, Ed25519Keypair, EphemeraKeypair};

#[derive(Debug, Clone, Parser)]
pub struct GenerateKeypairCmd;

impl GenerateKeypairCmd {
    pub async fn execute(self) {
        let keypair = Ed25519Keypair::generate(None);
        println!("Keypair hex: {:>5}", to_hex(keypair.to_raw_vec()));
        println!(
            "Public key hex: {:>5}",
            to_hex(keypair.public_key().to_raw_vec())
        );
    }
}
