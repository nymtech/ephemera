use network::codec::ProtoCodec;

use crate::app::signatures::backend::SignaturesBackend;
use crate::app::signatures::callback::SigningQuorumConsensusCallBack;
use crate::crypto::ed25519::Ed25519KeyPair;
use crate::network::node::NodeLauncher;
use crate::network::peer_discovery::{StaticPeerDiscovery, Topology};
use crate::protocols::implementations::gossip::full_gossip::{FullGossipError, FullGossipProtocol};
use crate::protocols::implementations::quorum_consensus::quorum::BasicQuorum;
use crate::protocols::implementations::quorum_consensus::quorum_consensus::{
    QuorumConsensusBroadcastProtocol, QuorumProtocolError,
};
use crate::protocols::protocol_handler::ProtocolHandler;
use crate::request::{FullGossip, RbMsg};
use crate::settings::Settings;

mod app;
mod cli;
mod crypto;
mod network;
mod protocols;
mod settings;

pub mod request {
    include!(concat!(env!("OUT_DIR"), "/broadcast.rs"));
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let args = cli::parse_args();
    let settings = Settings::load("configuration", args.config_file.as_str());

    let handle = NodeLauncher::with_settings(settings)
        .launch(move |rcv, tx, settings| {
            let handler = quorum_consensus_handler(settings);
            Box::pin(handler.run(rcv, tx))
        })
        .await;

    handle.await.unwrap();
}

fn quorum_consensus_handler(
    settings: Settings,
) -> ProtocolHandler<RbMsg, RbMsg, Vec<u8>, QuorumProtocolError> {
    //TODO: clarify relationship between quorum, topology and peer discovery
    let keypair = Ed25519KeyPair::from_hex(settings.private_key.as_ref()).unwrap();
    let topology = Topology::new(&settings);
    let peer_discovery = StaticPeerDiscovery::new(topology.clone());
    let backend = SignaturesBackend::new(&settings);
    let protocol = QuorumConsensusBroadcastProtocol::new(
        Box::new(peer_discovery),
        Box::new(BasicQuorum::new(topology.peers.len() + 1)),
        Box::new(SigningQuorumConsensusCallBack::new(keypair, backend)),
        settings,
    );
    ProtocolHandler::new(Box::new(protocol), ProtoCodec::new())
}

fn gossip_protocol_handler(
    topology: Topology,
) -> ProtocolHandler<FullGossip, FullGossip, Vec<u8>, FullGossipError> {
    let peer_discovery = StaticPeerDiscovery::new(topology);
    let protocol = FullGossipProtocol::new(Box::new(peer_discovery));
    ProtocolHandler::new(Box::new(protocol), ProtoCodec::new())
}
