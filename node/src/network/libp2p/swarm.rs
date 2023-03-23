use std::iter;
use std::sync::Arc;
use std::time::Duration;

use crate::broadcast::RbMsg;
use crate::config::Libp2pConfig;
use libp2p::core::{muxing::StreamMuxerBox, transport::Boxed};
use libp2p::gossipsub::{IdentTopic as Topic, MessageAuthenticity, ValidationMode};
use libp2p::swarm::NetworkBehaviour;
use libp2p::tcp::{tokio::Transport as TokioTransport, Config as TokioConfig};
use libp2p::yamux::YamuxConfig;
use libp2p::{gossipsub, noise, request_response, PeerId as Libp2pPeerId, Transport};

use crate::network::libp2p::behaviour::messages::{
    RbMsgMessagesCodec, RbMsgProtocol, RbMsgResponse,
};
use crate::network::libp2p::discovery::rendezvous;
use crate::network::libp2p::discovery::rendezvous::RendezvousBehaviour;
use crate::network::PeerDiscovery;
use crate::utilities::crypto::ed25519::Ed25519Keypair;

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "GroupBehaviourEvent")]
pub(crate) struct GroupNetworkBehaviour<P: PeerDiscovery> {
    pub(crate) rendezvous_behaviour: RendezvousBehaviour<P>,
    pub(crate) gossipsub: gossipsub::Behaviour,
    pub(crate) request_response: request_response::Behaviour<RbMsgMessagesCodec>,
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum GroupBehaviourEvent {
    Gossipsub(gossipsub::Event),
    RequestResponse(request_response::Event<RbMsg, RbMsgResponse>),
    Rendezvous(rendezvous::Event),
}

impl From<gossipsub::Event> for GroupBehaviourEvent {
    fn from(event: gossipsub::Event) -> Self {
        GroupBehaviourEvent::Gossipsub(event)
    }
}

impl From<request_response::Event<RbMsg, RbMsgResponse>> for GroupBehaviourEvent {
    fn from(event: request_response::Event<RbMsg, RbMsgResponse>) -> Self {
        GroupBehaviourEvent::RequestResponse(event)
    }
}

impl From<rendezvous::Event> for GroupBehaviourEvent {
    fn from(event: rendezvous::Event) -> Self {
        GroupBehaviourEvent::Rendezvous(event)
    }
}

//Create combined behaviour.
//Gossipsub takes care of message delivery semantics
//Peer discovery takes care of locating peers
pub(crate) fn create_behaviour<P: PeerDiscovery + 'static>(
    libp2p_conf: &Libp2pConfig,
    keypair: Arc<Ed25519Keypair>,
    peer_discovery: P,
) -> GroupNetworkBehaviour<P> {
    let ephemera_msg_topic = Topic::new(&libp2p_conf.ephemera_msg_topic_name);

    let mut gossipsub = create_gossipsub(keypair);
    gossipsub.subscribe(&ephemera_msg_topic).unwrap();

    let request_response = create_request_response();
    let rendezvous_behaviour = create_http_peer_discovery(peer_discovery);

    GroupNetworkBehaviour {
        rendezvous_behaviour,
        gossipsub,
        request_response,
    }
}

// Configure networking messaging stack(Gossipsub)
pub(crate) fn create_gossipsub(local_key: Arc<Ed25519Keypair>) -> gossipsub::Behaviour {
    let gossipsub_config = gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(5))
        .validation_mode(ValidationMode::Strict)
        .build()
        .expect("Valid config");

    gossipsub::Behaviour::new(
        MessageAuthenticity::Signed(local_key.0.clone()),
        gossipsub_config,
    )
    .expect("Correct configuration")
}

pub(crate) fn create_request_response() -> request_response::Behaviour<RbMsgMessagesCodec> {
    let config = Default::default();
    request_response::Behaviour::new(
        RbMsgMessagesCodec,
        iter::once((RbMsgProtocol, request_response::ProtocolSupport::Full)),
        config,
    )
}

pub(crate) fn create_http_peer_discovery<P: PeerDiscovery + 'static>(
    peer_discovery: P,
) -> RendezvousBehaviour<P> {
    RendezvousBehaviour::new(peer_discovery)
}

//Configure networking connection stack(Tcp, Noise, Yamux)
//Tcp protocol for networking
//Noise protocol for encryption
//Yamux protocol for multiplexing
pub(crate) fn create_transport(
    local_key: Arc<Ed25519Keypair>,
) -> Boxed<(Libp2pPeerId, StreamMuxerBox)> {
    let transport = TokioTransport::new(TokioConfig::default().nodelay(true));
    let noise_keypair = noise::Keypair::<noise::X25519Spec>::new()
        .into_authentic(&local_key.0.clone())
        .unwrap();
    let xx_config = noise::NoiseConfig::xx(noise_keypair);
    transport
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(xx_config.into_authenticated())
        .multiplex(YamuxConfig::default())
        .timeout(Duration::from_secs(20))
        .boxed()
}
