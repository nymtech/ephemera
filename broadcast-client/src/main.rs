use std::thread;

use bytes::BytesMut;
use prost_types::Timestamp;
use rand_chacha::rand_core::{RngCore, SeedableRng};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use uuid::Uuid;

use crate::broadcast::rb_msg::ReliableBroadcast::PrePrepare;
use crate::broadcast::{FullGossip, PrePrepareMsg, RbMsg};
use crate::crypto::{Ed25519KeyPair, KeyPair};

mod broadcast;
mod cli;
mod crypto;

#[tokio::main]
async fn main() {
    let args = cli::parse_args();
    if args.gossip {
        run_gossip().await;
    } else if args.broadcast {
        run_reliable_broadcast().await;
    } else if args.keypair {
        generate_keypair().await;
    } else {
        println!("No command specified, try cargo run -- --help");
    }
}

async fn generate_keypair() {
    let mut rng = rand::rngs::StdRng::from_entropy();
    let mut seed = [0u8; 32];
    rng.fill_bytes(&mut seed);
    let pair = Ed25519KeyPair::generate(&seed).unwrap();
    println!("Public key: {}", hex::encode(pair.verification_key));
    println!("Private key: {}", hex::encode(pair.signing_key));
}

async fn run_reliable_broadcast() {
    let mut conn = TcpStream::connect("127.0.0.1:3000").await.unwrap();
    loop {
        let mut message = quorum_message();
        conn.write_buf(&mut message).await.unwrap();
        thread::sleep(std::time::Duration::from_millis(3000));
    }
}

async fn run_gossip() {
    let mut conn = TcpStream::connect("127.0.0.1:3000").await.unwrap();
    loop {
        let mut message = gossip_message();
        conn.write_buf(&mut message).await.unwrap();
        thread::sleep(std::time::Duration::from_millis(3000));
    }
}

fn quorum_message() -> bytes::Bytes {
    let timestamp = Timestamp::from(std::time::SystemTime::now());
    let request = RbMsg {
        id: Uuid::new_v4().to_string(),
        node_id: "client".to_string(),
        timestamp: Some(timestamp),
        reliable_broadcast: Some(PrePrepare(PrePrepareMsg {
            payload: "Payload".bytes().collect(),
        })),
    };

    let mut buf = BytesMut::with_capacity(1028);
    prost::Message::encode_length_delimited(&request, &mut buf).unwrap();

    buf.freeze()
}

fn gossip_message() -> bytes::Bytes {
    let timestamp = Timestamp::from(std::time::SystemTime::now());
    let msg = FullGossip {
        id: Uuid::new_v4().to_string(),
        timestamp: Some(timestamp),
        payload: "Payload".bytes().collect(),
    };
    let mut buf = BytesMut::with_capacity(1028);
    prost::Message::encode_length_delimited(&msg, &mut buf).unwrap();

    buf.freeze()
}
