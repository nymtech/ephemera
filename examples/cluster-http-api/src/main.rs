// Test full HTTP API for each node
//
// Endpoints to test:
// * ephemera/node/health (GET) ✅
// * ephemera/node/block/{hash} (GET) ✅
// * ephemera/node/block/certificates/{hash} (GET) ✅
// * ephemera/node/block/height/{height} (GET) ✅
// * ephemera/node/block/last (GET) ✅
// * ephemera/node/config (GET) ✅
// * ephemera/node/dht/query (GET) ✅
// * ephemera/node/broadcast/info (GET)
// * ephemera/node/submit (POST) ✅
// * ephemera/node/dht/store (POST) ✅

use std::time::Duration;

use clap::Parser;
use log::info;

use ephemera::logging;

use crate::cluster::Cluster;

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

#[tokio::main]
async fn main() {
    logging::init_with_directives("info");

    let args = Args::parse();
    let cluster = Cluster::init(args).await;

    // SUBMIT MESSAGES - ephemera/node/submit
    let mut submit_messages = Box::pin(cluster.submit_messages_to_at_random_burst_and_wait(100));

    // QUERY LAST BLOCK - ephemera/node/block/last
    let mut last_block = Box::pin(cluster.query_last_block());

    // QUERY BLOCKS BY HEIGHT - ephemera/node/block/height/{height}
    let mut blocks_by_height = Box::pin(cluster.query_blocks_by_height());

    // QUERY BLOCKS BY HASH - ephemera/node/block/{hash}
    let mut block_hashes = Box::pin(cluster.query_blocks_by_hash());

    // STORE IN DHT - ephemera/node/dht/store
    let mut store_in_dht =
        Box::pin(cluster.store_in_dht_using_random_node(Duration::from_secs(10)));

    // QUERY DHT - ephemera/node/dht/query
    let mut query_dht = Box::pin(cluster.query_dht_using_random_node(Duration::from_secs(8)));

    tokio::select! {
        _ = &mut submit_messages => info!("Submit messages finished"),
        _ = &mut blocks_by_height => info!("Blocks by height finished"),
        _ = &mut block_hashes => info!("Block hashes finished"),
        _ = &mut last_block => info!("Last block finished"),
        _ = &mut store_in_dht => info!("Store in DHT finished"),
        _ = &mut query_dht => info!("Query DHT finished"),
    }

    info!("Cluster test finished");
}
