use tokio::task::JoinHandle;

use crate::app::signatures::backend::SignaturesBackend;
use crate::app::signatures::callback::SigningQuorumConsensusCallBack;
use crate::crypto::ed25519::{Ed25519KeyPair, KeyPair};
use crate::network::basic;
use crate::network::libp2p::gossipsub;
use crate::network::node::NodeLauncher;
use crate::network::peer_discovery::StaticPeerDiscovery;
use crate::network::Network;
use protocol::gossip::protocol::FullGossipProtocol;
use crate::protocol::Protocol;
use crate::protocol::protocol_handler::ProtocolHandler;
use crate::protocol::quorum_consensus::protocol::QuorumConsensusBroadcastProtocol;
use crate::protocol::quorum_consensus::quorum::BasicQuorum;
use crate::request::RbMsg;
use crate::settings::Settings;

mod app;
mod cli;
mod crypto;
mod network;
mod protocol;
mod settings;

pub mod request {
    include!(concat!(env!("OUT_DIR"), "/broadcast.rs"));
}

const CONFIG_DIR: &str = "configuration";

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let args = cli::parse_args();
    let settings = Settings::load(CONFIG_DIR, args.config_file.as_str());

    //Basic network
    let _network = basic::create_basic_network(&settings);

    //Or libp2p
    let network = gossipsub::create_swarm(&settings);

    //Basic floodsub
    let _protocol = gossip_protocol(&settings);

    //Or quorum consensus
    let protocol = quorum_consensus(settings.clone());

    launch(network, protocol, settings)
        .await
        .expect("Failed to launch node");
}

fn launch<
    N: Network + Send + 'static,
    M: prost::Message + Send + Sync + Default + 'static,
    P: Protocol<M> + Send + 'static,
>(
    network: N,
    protocol: P,
    settings: Settings,
) -> JoinHandle<()> {
    let protocol_handler = ProtocolHandler::new(protocol);
    NodeLauncher::launch(settings, network, protocol_handler)
}

fn quorum_consensus(settings: Settings) -> QuorumConsensusBroadcastProtocol<RbMsg, RbMsg> {
    let keypair = Ed25519KeyPair::generate().unwrap(); //TODO

    let backend = SignaturesBackend::new(&settings);
    let quorum = Box::new(BasicQuorum::new(settings.quorum.clone()));
    QuorumConsensusBroadcastProtocol::new(
        quorum,
        Box::new(SigningQuorumConsensusCallBack::new(keypair, backend)),
        settings,
    )
}

fn gossip_protocol(settings: &Settings) -> FullGossipProtocol {
    let _peer_discovery = StaticPeerDiscovery::new(settings);
    FullGossipProtocol::new()
}
