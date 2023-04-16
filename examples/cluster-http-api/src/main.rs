use std::time::Duration;

use clap::Parser;
use log::info;

use ephemera::crypto::EphemeraKeypair;
use ephemera::helpers::init_logging_with_directives;

mod cluster;
mod node;
mod util;

#[derive(Parser, Clone)]
#[command(name = "cluster-http-api")]
#[command(about = "Ephemera cluster http, meant to test cluster behaviour over http api.", long_about = None)]
#[command(next_line_help = true)]
struct Args {
    /// Ephemera cluster size
    #[clap(long)]
    nr_of_nodes: usize,
    /// Messages submitting frequency in ms
    #[clap(long, default_value_t = 3000)]
    messages_post_frequency_ms: u64,
}

//1. Submit messages to different nodes
//2. Use full HTTP API for each node

#[tokio::main]
async fn main() {
    init_logging_with_directives("info");

    let keypair = ephemera::crypto::Keypair::generate(None);
    let args = Args::parse();

    let mut nodes = vec![];
    for i in 0..args.nr_of_nodes {
        let node = node::Node::init(i, format!("http://127.0.0.1:700{}", i)).await;
        nodes.push(node);
    }

    for node in nodes.iter_mut() {
        info!("Node {} last block: {}", node.id, node.last_block);
    }

    let mut cluster = cluster::Cluster::new(args.clone(), nodes, keypair);

    let mut submit_messages_handle = cluster
        .submit_messages_to_at_random_burst_and_wait(100)
        .await
        .unwrap();

    let mut query_blocks_by_height_handle = cluster.query_blocks_by_height().await.unwrap();

    let mut query_block_hashes_handle = cluster.query_blocks_by_hash().await.unwrap();

    let mut store_in_dht_handle = cluster
        .store_in_dht_using_random_node(Duration::from_secs(10))
        .await
        .unwrap();

    let mut query_dht_handle = cluster
        .query_dht_using_random_node(Duration::from_secs(8))
        .await
        .unwrap();

    tokio::select! {
        _ = &mut submit_messages_handle => {
            info!("Submit messages task exited");
        }
        _ = &mut query_blocks_by_height_handle => {
            info!("Query blocks by height task exited");
        }
        _ = &mut query_block_hashes_handle => {
            info!("Query blocks by hash task exited");
        }
        _ = &mut store_in_dht_handle => {
            info!("Store in dht task exited");
        }
        _ = &mut query_dht_handle => {
            info!("Query dht task exited");
        }
    }

    info!("Cluster started");
}
