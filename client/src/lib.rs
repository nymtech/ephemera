pub mod cli;

use std::thread;
use bytes::BytesMut;
use prost_types::Timestamp;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use uuid::Uuid;
use ephemera::request::rb_msg::ReliableBroadcast::PrePrepare;
use ephemera::request::{PrePrepareMsg, RbMsg};

pub async fn run_reliable_broadcast<F: Fn() -> Vec<u8>>(node_address: String, sleep_time_sec: u64, payload_generator: F) {
    let mut conn = TcpStream::connect(node_address).await.unwrap();
    loop {
        let payload = payload_generator();
        let mut message = quorum_message(payload);
        conn.write_buf(&mut message).await.unwrap();
        thread::sleep(std::time::Duration::from_secs(sleep_time_sec));
    }
}

fn quorum_message(payload: Vec<u8>) -> bytes::Bytes {
    let timestamp = Timestamp::from(std::time::SystemTime::now());
    let request = RbMsg {
        id: Uuid::new_v4().to_string(),
        node_id: "client".to_string(),
        timestamp: Some(timestamp),
        reliable_broadcast: Some(PrePrepare(PrePrepareMsg { payload })),
    };

    println!("Sending request {:?}", request);

    let mut buf = BytesMut::with_capacity(1028);
    prost::Message::encode_length_delimited(&request, &mut buf).unwrap();

    buf.freeze()
}