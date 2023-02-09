use std::sync::{Arc, Mutex};
use std::thread;

use clap::Parser;

use ephemera::api::types::{ApiBlock, ApiKeypair, ApiSignedMessage};
use ephemera::logging::init_logging;
use ephemera::utilities::CryptoApi;

use crate::http_client::SignedMessageClient;
use crate::ws_listener::WsBlockListener;

mod http_client;
mod ws_listener;

#[derive(Parser, Clone)]
#[command(name = "ephemera-http-ws-example")]
#[command(about = "Ephemera http and ws external interfaces example", long_about = None)]
#[command(next_line_help = true)]
struct Args {
    #[clap(long)]
    host: String,
    #[clap(long)]
    http_port: String,
    #[clap(long)]
    ws_port: String,
    #[clap(long, default_value_t = 1000)]
    messages_frequency_ms: u64,
    #[clap(long, default_value_t = 10)]
    block_query_frequency_sec: u64,
}

#[derive(Default)]
struct Data {
    received_blocks: Vec<ApiBlock>,
    sent_messages: Vec<ApiSignedMessage>,
}

#[tokio::main]
async fn main() {
    init_logging();

    let args = Args::parse();

    let keypair = CryptoApi::generate_keypair().unwrap();
    println!("Generated new keypair:");
    println!("Private key: {:?>5}", keypair.private_key);
    println!("Public key: {:?>5}\n", keypair.public_key);

    println!("Connecting to ephemera node on {}\n", args.host);

    let data: Data = Default::default();
    let shared_data = Arc::new(Mutex::new(data));

    let signed_msg_data = shared_data.clone();
    let signed_msg_args = args.clone();
    tokio::spawn(async move {
        send_signed_messages(keypair, signed_msg_data, signed_msg_args).await;
    });

    let compare_ws_http_data = shared_data.clone();
    let compare_ws_http_args = args.clone();
    tokio::spawn(async move {
        compare_ws_http_blocks(compare_ws_http_data, compare_ws_http_args).await;
    });

    let listen_ws_data = shared_data.clone();
    let listen_ws_args = args.clone();
    tokio::spawn(async move {
        listen_ws_blocks(listen_ws_data, listen_ws_args).await;
    })
    .await
    .unwrap();
}

async fn listen_ws_blocks(shared_data: Arc<Mutex<Data>>, args: Args) {
    let ws_url = format!("ws://{}:{}", args.host, args.ws_port);
    println!("Listening to ws blocks on {ws_url}\n",);
    let mut listener = WsBlockListener::new(ws_url, shared_data);
    listener.listen().await;
}

async fn send_signed_messages(keypair: ApiKeypair, shared_data: Arc<Mutex<Data>>, args: Args) -> ! {
    let http_url = format!("http://{}:{}", args.host, args.http_port);
    println!(
        "Sending signed messages every {} ms\n",
        args.messages_frequency_ms
    );
    let mut client = SignedMessageClient::new(http_url, shared_data);
    let mut counter = 0;
    loop {
        let msg = client.signed_message(keypair.clone(), format!("Epoch {}", counter)).await;
        client.send_message(msg).await;
        counter += 1;
        thread::sleep(std::time::Duration::from_millis(args.messages_frequency_ms));
    }
}

async fn compare_ws_http_blocks(shared_data: Arc<Mutex<Data>>, args: Args) -> ! {
    let http_url = format!("http://{}:{}", args.host, args.http_port);
    println!(
        "Querying new blocks {} ms\n",
        args.block_query_frequency_sec
    );

    let client = SignedMessageClient::new(http_url, shared_data);
    loop {
        let blocks: Vec<ApiBlock> = client
            .data
            .lock()
            .unwrap()
            .received_blocks
            .drain(..)
            .collect();
        if !blocks.is_empty() {
            println!("Received nr of blocks: {:?}\n", blocks.len());
        }
        for block in blocks {
            match client.block_by_id(block.header.id.clone()).await {
                None => {
                    println!("Block not found: {:?}\n", block.header.id.clone());
                }
                Some(http_block) => {
                    compare_blocks(&block, &http_block);
                }
            }

            println!("Verifying block signature");
            if let Err(err) = CryptoApi::verify(&block.as_raw_block(), &block.signature.clone()) {
                println!("Block signature mismatch: {err:?}\n",);
            }

            println!("Verifying messages signatures");
            for message in block.signed_messages {
                let signature = message.signature.clone();
                if let Err(err) = CryptoApi::verify(&message.into_raw_message(), &signature) {
                    println!("Message signature mismatch: {err:?}\n",);
                }
            }

            println!();
        }
        thread::sleep(std::time::Duration::from_millis(
            args.block_query_frequency_sec,
        ));
    }
}

fn compare_blocks(block: &ApiBlock, http_block: &ApiBlock) {
    println!("Comparing block {}\n", block.header.id);

    let ws_block_header = &block.header;
    let http_block_header = &http_block.header;
    if ws_block_header != http_block_header {
        println!("Block header mismatch");
        println!("WS: {ws_block_header:?}",);
        println!("HTTP: {http_block_header:?}",);
    } else {
        println!("Block header match");
    }

    let ws_block_signature = &block.signature;
    let http_block_signature = &http_block.signature;
    if ws_block_signature != http_block_signature {
        println!("Block signature mismatch");
        println!("WS: {ws_block_signature:?}",);
        println!("HTTP: {http_block_signature:?}");
    } else {
        println!("Block signature match");
    }

    println!(
        "Comparing block messages, count: {}",
        block.signed_messages.len()
    );
    let ws_block_messages = &block.signed_messages;
    let http_block_messages = &http_block.signed_messages;
    if ws_block_messages != http_block_messages {
        println!("Block messages mismatch");
        println!("WS: {ws_block_messages:?}");
        println!("HTTP: {http_block_messages:?}");
    } else {
        println!("Block messages match");
    }
}
