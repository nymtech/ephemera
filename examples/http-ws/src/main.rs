use std::sync::{Arc, Mutex};
use std::thread;

use clap::Parser;

use ephemera::peer::ToPeerId;
use ephemera::{
    codec::Encode,
    crypto::{EphemeraKeypair, EphemeraPublicKey, Keypair},
    ephemera_api::{ApiBlock, ApiCertificate, ApiEphemeraMessage, RawApiEphemeraMessage},
};

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
    sent_messages: Vec<ApiEphemeraMessage>,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let args = Args::parse();

    println!("Connecting to ephemera node on {}\n", args.host);

    let data: Data = Default::default();
    let shared_data = Arc::new(Mutex::new(data));

    let signed_msg_data = shared_data.clone();
    let signed_msg_args = args.clone();
    tokio::spawn(async move {
        let keypair = Keypair::generate(None);
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

async fn send_signed_messages(keypair: Keypair, shared_data: Arc<Mutex<Data>>, args: Args) -> ! {
    let http_url = format!("http://{}:{}", args.host, args.http_port);
    println!("Sending messages to {http_url}\n",);
    println!(
        "Sending signed messages every {} ms\n",
        args.messages_frequency_ms
    );
    let mut client = SignedMessageClient::new(http_url, shared_data);
    let mut counter = 0;
    let keypair = Arc::new(keypair);
    loop {
        let msg = client
            .signed_message(keypair.clone(), format!("Epoch {counter}",))
            .await;

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
            match client.block_by_hash(block.hash()).await {
                None => {
                    println!("Block not found by hash: {:?}\n", block.hash());
                    continue;
                }
                Some(http_block) => {
                    println!("Block found by hash: {:?}\n", block.hash());
                    compare_blocks(&block, &http_block);

                    if verify_messages_signatures(block.clone()).is_ok() {
                        println!("All messages signatures are valid");
                    } else {
                        println!("Some messages signatures are invalid");
                    }

                    match client.block_certificates(block.hash()).await {
                        None => {
                            println!("Block certificates not found by hash: {:?}\n", block.hash());
                        }
                        Some(certificates) => {
                            println!(
                                "Block certificates found by hash: {:?}, len {:?}\n",
                                block.hash(),
                                certificates.len()
                            );
                            verify_block_certificates(&block, certificates).unwrap();
                        }
                    }
                }
            }

            println!();
        }
        thread::sleep(std::time::Duration::from_secs(
            args.block_query_frequency_sec,
        ));
    }
}

fn verify_messages_signatures(block: ApiBlock) -> anyhow::Result<()> {
    println!(
        "Verifying messages signatures: {:?}\n",
        block.messages.len()
    );
    for message in block.messages {
        let signature = message.certificate.clone();
        let message: RawApiEphemeraMessage = message.into();
        let data = message.encode()?;

        if !signature.public_key.verify(&data, &signature.signature) {
            anyhow::bail!("Signature verification failed");
        }
    }
    Ok(())
}

fn verify_block_certificates(
    block: &ApiBlock,
    certificates: Vec<ApiCertificate>,
) -> anyhow::Result<()> {
    println!("Verifying block certificates: {:?}\n", certificates.len());
    for certificate in certificates {
        match block.verify(&certificate) {
            Ok(valid) => {
                if valid {
                    println!(
                        "Certificate from peer {} is valid",
                        certificate.public_key.peer_id()
                    );
                } else {
                    println!(
                        "Certificate from peer {} is invalid",
                        certificate.public_key.peer_id()
                    );
                }
            }
            Err(err) => {
                println!("Certificate verification failed: {err:?}",);
            }
        }
    }
    Ok(())
}

fn compare_blocks(block: &ApiBlock, http_block: &ApiBlock) {
    println!("Comparing block {}\n", block.hash());

    let ws_block_header = &block.header;
    let http_block_header = &http_block.header;
    if ws_block_header != http_block_header {
        println!("Block header mismatch");
        println!("WS: {ws_block_header:?}",);
        println!("HTTP: {http_block_header:?}",);
    } else {
        println!("Block header match");
    }

    println!("Comparing block messages, count: {}", block.messages.len());
    let ws_block_messages = &block.messages;
    let http_block_messages = &http_block.messages;
    if ws_block_messages != http_block_messages {
        println!("Block messages mismatch");
        println!("WS: {ws_block_messages:?}");
        println!("HTTP: {http_block_messages:?}");
    } else {
        println!("Block messages match");
    }
}
