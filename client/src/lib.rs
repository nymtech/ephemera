pub mod cli;

use bytes::BytesMut;
use ephemera::request::rb_msg::ReliableBroadcast::PrePrepare;
use ephemera::request::{PrePrepareMsg, RbMsg};
use prost_types::Timestamp;
use tokio::io::{AsyncRead, AsyncWriteExt, AsyncReadExt};
use tokio::net::TcpStream;
use uuid::Uuid;

pub struct RbClient<R> {
    pub payload_stream: R,
    pub node_address: String,
}

impl<R: AsyncRead + Unpin> RbClient<R> {
    pub fn new(
        node_address: String,
        payload_stream: R,
    ) -> Self {
        RbClient {
            payload_stream,
            node_address,
        }
    }

    pub async fn run_reliable_broadcast(&mut self) {
        let mut conn = TcpStream::connect(&self.node_address).await.unwrap();
        loop {
            let mut buf = BytesMut::new();
            self.payload_stream.read_buf(&mut buf).await.unwrap();
            let payload = buf.to_vec();
            let mut message = quorum_message(payload);
            conn.write_buf(&mut message).await.unwrap();
        }
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
