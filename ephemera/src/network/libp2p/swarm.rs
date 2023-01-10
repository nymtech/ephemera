use std::str::FromStr;
use std::time::Duration;

use async_trait::async_trait;
use bytes::BytesMut;
use futures::StreamExt;
use libp2p::core::{muxing::StreamMuxerBox, transport::Boxed};
use libp2p::gossipsub::{
    Gossipsub, GossipsubConfigBuilder, GossipsubEvent, IdentTopic as Topic, MessageAuthenticity,
    ValidationMode,
};
use libp2p::mplex::MplexConfig;
use libp2p::noise::NoiseAuthenticated;
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::tcp::{tokio::Transport as TokioTransport, Config as TokioConfig};
use libp2p::yamux::YamuxConfig;
use libp2p::{identity::Keypair, Multiaddr, PeerId, Swarm, Transport};
use tokio::select;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_util::codec::Decoder;
use tokio_util::codec::Encoder;

use crate::broadcast_protocol::ProtocolRequest;
use crate::config::configuration::Configuration;
use crate::network::codec::ProtoCodec;
use crate::network::peer_discovery::StaticPeerDiscovery;
use crate::network::{BroadcastMessage, Network};
use crate::request::RbMsg;

pub struct SwarmNetwork {
    config: Configuration,
    swarm: Swarm<GroupNetworkBehaviour>,
    to_protocol: Sender<ProtocolRequest>,
    from_protocol: Receiver<BroadcastMessage<RbMsg>>,
}

impl SwarmNetwork {
    pub fn new(
        conf: Configuration,
        to_protocol: Sender<ProtocolRequest>,
        from_protocol: Receiver<BroadcastMessage<RbMsg>>,
    ) -> SwarmNetwork {
        let swarm = SwarmNetwork::create_swarm(conf.clone());
        SwarmNetwork {
            config: conf,
            swarm,
            to_protocol,
            from_protocol,
        }
    }
    //Message delivery and peer discovery
    fn create_swarm(conf: Configuration) -> Swarm<GroupNetworkBehaviour> {
        let sec_key = hex::decode(&conf.priv_key).unwrap();
        let local_key = Keypair::from_protobuf_encoding(&sec_key[..]).unwrap();
        let local_id = PeerId::from(local_key.public());

        let transport = create_transport(&local_key);
        let behaviour = create_behaviour(conf, local_key);

        Swarm::with_tokio_executor(transport, behaviour, local_id)
    }
}

#[async_trait]
impl Network for SwarmNetwork {
    async fn run(mut self) {
        let address = Multiaddr::from_str(self.config.address.as_str()).expect("Invalid multi-address");
        self.swarm.listen_on(address).unwrap();

        let mut codec = ProtoCodec::<RbMsg, RbMsg>::new();
        let topic = Topic::new(self.config.libp2p.topic_name);
        loop {
            select!(
                // Handle incoming messages from the network
                swarm_event = self.swarm.select_next_some() => {
                    handle_incoming_messages(&self.to_protocol, &mut codec, swarm_event).await;
                },
                // Handle messages from broadcast_protocol
                br = self.from_protocol.recv() => {
                    match br {
                        Some(b) => {
                            send_message(b, &mut self.swarm.behaviour_mut().gossipsub, &topic,  &mut codec).await;
                        }
                        None => {
                            log::error!("Broadcast channel closed"); break;
                        }
                    }
                }
            );
        }
    }
}

#[allow(clippy::collapsible_match, clippy::single_match)]
async fn handle_incoming_messages<E>(
    to_network: &Sender<ProtocolRequest>,
    codec: &mut ProtoCodec<RbMsg, RbMsg>,
    swarm_event: SwarmEvent<GroupBehaviourEvent, E>,
) {
    match swarm_event {
        SwarmEvent::Behaviour(b) => match b {
            GroupBehaviourEvent::Gossipsub(gs) => match gs {
                GossipsubEvent::Message {
                    propagation_source,
                    message_id: _,
                    message,
                } => {
                    let mut data = BytesMut::from(&message.data[..]);
                    if let Ok(Some(msg)) = codec.decode(&mut data) {
                        to_network
                            .send(ProtocolRequest::new(propagation_source.to_string(), msg))
                            .await
                            .unwrap();
                    } else {
                        log::error!("Failed to decode message");
                    }
                }
                _ => {}
            },
            //Ignore other NetworkBehaviour events for now
            _ => {}
        },
        //Ignore other Swarm events for now
        _ => {}
    }
}

async fn send_message<R: Default + prost::Message, B: prost::Message>(
    msg: BroadcastMessage<B>,
    gossipsub: &mut Gossipsub,
    topic: &Topic,
    codec: &mut ProtoCodec<R, B>,
) {
    let mut buf = BytesMut::new();
    if codec.encode(msg.message, &mut buf).is_ok() {
        if let Err(err) = gossipsub.publish(topic.clone(), buf) {
            log::error!("Error publishing message: {}", err);
        }
    };
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "GroupBehaviourEvent")]
pub struct GroupNetworkBehaviour {
    pub gossipsub: Gossipsub,
    pub peer_discovery: StaticPeerDiscovery,
}

#[allow(clippy::large_enum_variant)]
pub enum GroupBehaviourEvent {
    Gossipsub(GossipsubEvent),
    StaticPeerDiscovery(()),
}

impl From<GossipsubEvent> for GroupBehaviourEvent {
    fn from(event: GossipsubEvent) -> Self {
        GroupBehaviourEvent::Gossipsub(event)
    }
}

impl From<()> for GroupBehaviourEvent {
    fn from(event: ()) -> Self {
        GroupBehaviourEvent::StaticPeerDiscovery(event)
    }
}

//Create combined behaviour.
//Gossipsub takes care of message delivery semantics
//Peer discovery takes care of locating peers
fn create_behaviour(conf: Configuration, local_key: Keypair) -> GroupNetworkBehaviour {
    let mut gossipsub = create_gossipsub(local_key);
    let topic = Topic::new(&conf.libp2p.topic_name);
    gossipsub.subscribe(&topic).unwrap();

    let peer_discovery = StaticPeerDiscovery::new(conf);

    for peer in peer_discovery.peer_ids() {
        log::debug!("Adding peer: {}", peer);
        gossipsub.add_explicit_peer(&peer);
    }

    GroupNetworkBehaviour {
        gossipsub,
        peer_discovery,
    }
}

//Configure networking messaging stack(Gossipsub)
fn create_gossipsub(local_key: Keypair) -> Gossipsub {
    let gossipsub_config = GossipsubConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(1))
        .validation_mode(ValidationMode::Strict)
        .build()
        .expect("Valid config");

    Gossipsub::new(MessageAuthenticity::Signed(local_key), gossipsub_config).expect("Correct configuration")
}

//Configure networking connection stack(Tcp, Noise, Yamux)
//Tcp protocol for networking
//Noise protocol for encryption
//Yamux protocol for multiplexing
fn create_transport(local_key: &Keypair) -> Boxed<(PeerId, StreamMuxerBox)> {
    let transport = TokioTransport::new(TokioConfig::default().nodelay(true));
    transport
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(NoiseAuthenticated::xx(local_key).unwrap())
        .multiplex(libp2p::core::upgrade::SelectUpgrade::new(
            YamuxConfig::default(),
            MplexConfig::default(),
        ))
        .timeout(Duration::from_secs(20))
        .boxed()
}
