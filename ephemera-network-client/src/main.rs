use std::{env, thread};

use bytes::BytesMut;
use prost_types::Timestamp;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use uuid::Uuid;

use crate::broadcast::rb_msg::ReliableBroadcast::PrePrepare;
use crate::broadcast::{PrePrepareMsg, RbMsg};
use crate::libp2p2_crypto::{KeyPair, Libp2pKeypair};

mod broadcast;
mod cli;
mod command;
mod crypto;
mod libp2p2_crypto;

#[tokio::main]
async fn main() {
    if !env::vars().any(|(k, _)| k == "RUST_LOG") {
        env::set_var("RUST_LOG", "debug");
    }
    let args = cli::parse_args();
    if args.broadcast {
        run_reliable_broadcast().await;
    } else if args.keypair {
        generate_keypair().await;
    } else {
        println!("No command specified, try cargo run -- --help");
    }
}

async fn generate_keypair() {
    let pair = Libp2pKeypair::generate(&[]).unwrap();
    println!(
        "Public key: {}",
        hex::encode(pair.0.public().to_protobuf_encoding())
    );
    println!(
        "Private key: {}",
        hex::encode(pair.0.to_protobuf_encoding().unwrap())
    );
}

async fn run_reliable_broadcast() {
    let mut conn = TcpStream::connect("127.0.0.1:4001").await.unwrap();
    let mut c = 1;
    loop {
        let mut message = quorum_message();
        conn.write_buf(&mut message).await.unwrap();
        println!("Sent message {}", c);
        c += 1;
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
