use ephemera_client::cli::Commands;
use ephemera_client::{cli, RbClient};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::thread;
use tokio::io::{AsyncRead, ReadBuf};

#[tokio::main]
async fn main() {
    let args = cli::parse_args();
    match args.command {
        Commands::Broadcast { node_address } => {
            let mut client = RbClient::new(node_address, PayloadStream);
            client.run_reliable_broadcast().await;
        }
    }
}

struct PayloadStream;

impl AsyncRead for PayloadStream {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        thread::sleep(std::time::Duration::from_millis(3000));
        buf.put_slice(b"hello world");
        Poll::Ready(Ok(()))
    }
}
