use std::time::Duration;

use clap::Parser;

use ephemera::crypto::EphemeraKeypair;
use ephemera::helpers::{init_logging_with_directives};

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
    nr_of_nodes: u64,
    /// Messages submitting frequency in ms
    #[clap(long, default_value_t = 3000)]
    messages_post_frequency_ms: u64,
    /// Block query frequency in sec
    #[clap(long, default_value_t = 30)]
    block_query_frequency_sec: u64,
    /// Block production interval in sec
    #[clap(long)]
    node_block_production_interval: u64,
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
        log::info!("Node {} last block: {}", node.id, node.last_block);
    }

    let mut cluster = cluster::Cluster::new(args.clone(), nodes, keypair);

    let messages_interval = Duration::from_millis(args.messages_post_frequency_ms);
    let blocks_by_height_interval = Duration::from_secs(args.block_query_frequency_sec);

    let mut submit_messages_handle = cluster
        .submit_messages_to_all_nodes_at_the_same_interval(messages_interval)
        .await
        .unwrap();
    let mut query_blocks_by_height_handle = cluster
        .query_blocks_by_height(blocks_by_height_interval)
        .await
        .unwrap();

    tokio::select! {
        _ = &mut submit_messages_handle => {
            log::info!("Submit messages task exited");
        }
        _ = &mut query_blocks_by_height_handle => {
            log::info!("Query blocks by height task exited");
        }
    }

    log::info!("Cluster started");
}
