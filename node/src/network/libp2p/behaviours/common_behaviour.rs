use std::iter;
use std::sync::Arc;
use std::time::Duration;

use libp2p::core::{muxing::StreamMuxerBox, transport::Boxed};
use libp2p::gossipsub::{IdentTopic as Topic, MessageAuthenticity, ValidationMode};
use libp2p::kad;
use libp2p::swarm::NetworkBehaviour;
use libp2p::tcp::{tokio::Transport as TokioTransport, Config as TokioConfig};
use libp2p::yamux::YamuxConfig;
use libp2p::{gossipsub, noise, request_response, PeerId as Libp2pPeerId, Transport};

use crate::broadcast::RbMsg;
use crate::crypto::Keypair;
use crate::network::discovery::PeerDiscovery;
use crate::network::libp2p::behaviours::broadcast_messages::{
    RbMsgMessagesCodec, RbMsgProtocol, RbMsgResponse,
};
use crate::network::libp2p::behaviours::rendezvous;
use crate::network::libp2p::behaviours::rendezvous::RendezvousBehaviour;
use crate::network::peer::ToPeerId;
use crate::utilities::hash::{EphemeraHasher, Hasher};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "GroupBehaviourEvent")]
pub(crate) struct GroupNetworkBehaviour<P: PeerDiscovery> {
    pub(crate) rendezvous_behaviour: RendezvousBehaviour<P>,
    pub(crate) gossipsub: gossipsub::Behaviour,
    pub(crate) request_response: request_response::Behaviour<RbMsgMessagesCodec>,
    pub(crate) kademlia: kad::Kademlia<kad::store::MemoryStore>,
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum GroupBehaviourEvent {
    Gossipsub(gossipsub::Event),
    RequestResponse(request_response::Event<RbMsg, RbMsgResponse>),
    Rendezvous(rendezvous::Event),
    Kademlia(kad::KademliaEvent),
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

impl From<kad::KademliaEvent> for GroupBehaviourEvent {
    fn from(event: kad::KademliaEvent) -> Self {
        GroupBehaviourEvent::Kademlia(event)
    }
}

//Create combined behaviour.
//Gossipsub takes care of message delivery semantics
//Peer discovery takes care of locating peers
pub(crate) fn create_behaviour<P: PeerDiscovery + 'static>(
    keypair: Arc<Keypair>,
    ephemera_msg_topic: Topic,
    peer_discovery: P,
) -> GroupNetworkBehaviour<P> {
    let gossipsub = create_gossipsub(keypair.clone(), &ephemera_msg_topic);
    let request_response = create_request_response();
    let rendezvous_behaviour = create_http_peer_discovery(peer_discovery);
    let kademlia = create_kademlia(keypair);

    GroupNetworkBehaviour {
        rendezvous_behaviour,
        gossipsub,
        request_response,
        kademlia,
    }
}

// Configure networking messaging stack(Gossipsub)
pub(crate) fn create_gossipsub(local_key: Arc<Keypair>, topic: &Topic) -> gossipsub::Behaviour {
    let gossipsub_config = gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(5))
        .message_id_fn(|msg: &gossipsub::Message| Hasher::digest(&msg.data).into())
        .validation_mode(ValidationMode::Strict)
        .build()
        .expect("Valid config");

    let mut behaviour = gossipsub::Behaviour::new(
        MessageAuthenticity::Signed(local_key.0.clone()),
        gossipsub_config,
    )
    .expect("Correct configuration");
    behaviour.subscribe(topic).expect("Valid topic");
    behaviour
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

pub(super) fn create_kademlia(local_key: Arc<Keypair>) -> kad::Kademlia<kad::store::MemoryStore> {
    let peer_id = local_key.peer_id();
    let mut cfg = kad::KademliaConfig::default();
    cfg.set_query_timeout(Duration::from_secs(5 * 60));
    let store = kad::store::MemoryStore::new(peer_id.0);
    kad::Kademlia::with_config(peer_id.0, store, cfg)
}

//Configure networking connection stack(Tcp, Noise, Yamux)
//Tcp protocol for networking
//Noise protocol for encryption
//Yamux protocol for multiplexing
pub(crate) fn create_transport(local_key: Arc<Keypair>) -> Boxed<(Libp2pPeerId, StreamMuxerBox)> {
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
