use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::time::Duration;

use async_trait::async_trait;
use bytes::BytesMut;
use futures::StreamExt;
use libp2p::core::{muxing::StreamMuxerBox, transport::Boxed};
use libp2p::gossipsub::{
    Gossipsub, GossipsubConfigBuilder, GossipsubEvent, GossipsubMessage, IdentTopic as Topic,
    MessageAuthenticity, MessageId, ValidationMode,
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

use crate::network::basic::listener::NetworkPacket;
use crate::network::codec::ProtoCodec;
use crate::network::peer_discovery::{PeerDiscovery, StaticPeerDiscovery};
use crate::network::{BroadcastMessage, Network};
use crate::settings::Settings;

#[async_trait]
impl Network for Swarm<GroupNetworkBehaviour> {
    async fn start<R: prost::Message + Default + 'static, B: prost::Message + Default>(
        mut self,
        settings: Settings,
        to_network: Sender<NetworkPacket<R>>,
        mut rcv_br: Receiver<BroadcastMessage<B>>,
    ) {
        let address = Multiaddr::from_str(&settings.address).expect("Invalid multi-address");
        self.listen_on(address).unwrap();

        let mut codec = ProtoCodec::<R, B>::new();
        let topic = Topic::new(settings.gossipsub.topic_name);

        loop {
            select!(
                // Handle incoming messages from the network
                swarm_event = self.select_next_some() => {
                    match swarm_event {
                        SwarmEvent::Behaviour(b) => match b {
                            GroupBehaviourEvent::Gossipsub(gs) => match gs {
                                GossipsubEvent::Message {
                                    propagation_source,
                                    message_id: _,
                                    message,
                                } => {
                                    let mut data = BytesMut::from(&message.data[..]);
                                    if let Ok(Some(msg)) = codec.decode(&mut data){
                                         to_network
                                                .send(NetworkPacket::new(propagation_source.to_string(), msg))
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
                // Handle messages from protocol
                br = rcv_br.recv() => {
                    match br {
                        Some(b) => {
                            let mut buf = BytesMut::new();
                            if codec.encode(b.message, &mut buf).is_ok(){
                                if let Err(err) = self.behaviour_mut().gossipsub.publish(topic.clone(), buf){
                                    log::error!("Error publishing message: {}", err);
                                }
                            };
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

//Combine networking stack and network protocol
pub fn create_swarm(settings: &Settings) -> Swarm<GroupNetworkBehaviour> {
    let sec_key = hex::decode(&settings.private_key).unwrap();
    let local_key = Keypair::from_protobuf_encoding(&sec_key[..]).unwrap();
    let local_id = PeerId::from(local_key.public());

    let transport = create_transport(&local_key);

    let behaviour = create_behaviour(&settings, local_key);

    Swarm::with_tokio_executor(transport, behaviour, local_id)
}

//Create combined behaviour.
//Gossipsub takes care of message delivery semantics
//Static peer discovery takes care of locating peers, in future this will be replaced with a DHT
fn create_behaviour(settings: &&Settings, local_key: Keypair) -> GroupNetworkBehaviour {
    let mut gossipsub = create_gossipsub(local_key);
    let topic = Topic::new(&settings.gossipsub.topic_name);
    gossipsub.subscribe(&topic).unwrap();

    let peer_discovery = StaticPeerDiscovery::new(settings);

    for peer in peer_discovery.peer_ids() {
        log::debug!("Adding peer: {}", peer);
        gossipsub.add_explicit_peer(&peer);
    }

    GroupNetworkBehaviour {
        gossipsub,
        peer_discovery,
    }
}

//Configure networking protocol stack(Gossipsub)
fn create_gossipsub(local_key: Keypair) -> Gossipsub {
    let message_id_fn = |message: &GossipsubMessage| {
        let mut s = DefaultHasher::new();
        message.data.hash(&mut s);
        MessageId::from(s.finish().to_string())
    };

    let gossipsub_config = GossipsubConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10))
        .validation_mode(ValidationMode::Strict)
        .message_id_fn(message_id_fn)
        .build()
        .expect("Valid config");

    Gossipsub::new(MessageAuthenticity::Signed(local_key), gossipsub_config).expect("Correct configuration")
}

//Configure networking connection stack(Tcp, Noise, Yamux)
fn create_transport(local_key: &Keypair) -> Boxed<(PeerId, StreamMuxerBox)> {
    let transport = TokioTransport::new(TokioConfig::default().nodelay(true));
    transport
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(NoiseAuthenticated::xx(local_key).unwrap())
        .multiplex(libp2p::core::upgrade::SelectUpgrade::new(
            YamuxConfig::default(),
            MplexConfig::default(),
        ))
        .timeout(std::time::Duration::from_secs(20))
        .boxed()
}
