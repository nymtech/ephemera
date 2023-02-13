use std::borrow::Borrow;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use libp2p::core::{muxing::StreamMuxerBox, transport::Boxed};
use libp2p::gossipsub::{
    Gossipsub, GossipsubConfigBuilder, GossipsubEvent, IdentTopic as Topic, MessageAuthenticity,
    ValidationMode,
};
use libp2p::mplex::MplexConfig;
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::tcp::{tokio::Transport as TokioTransport, Config as TokioConfig};
use libp2p::yamux::YamuxConfig;
use libp2p::{identity::Keypair, noise, Multiaddr, PeerId as Libp2pPeerId, Swarm, Transport};
use tokio::select;

use crate::block::SignedMessage;
use crate::broadcast::RbMsg;
use crate::config::configuration::Configuration;
use crate::network::libp2p::listener::{
    BroadcastReceiverHandle, MessageReceiverHandle, MessagesReceiver, NetworkBroadcastReceiver,
    NetworkMessages,
};
use crate::network::libp2p::peer_discovery::StaticPeerDiscovery;
use crate::network::libp2p::sender::{EphemeraMessagesNotifier, EphemeraMessagesReceiver};
use crate::utilities::crypto::libp2p2_crypto::Libp2pKeypair;

pub(crate) struct NetworkMessage<T> {
    pub(crate) msg: T,
}

impl<T> NetworkMessage<T> {
    pub(crate) fn new(msg: T) -> Self {
        NetworkMessage { msg }
    }
}

pub struct SwarmNetwork {
    config: Configuration,
    swarm: Swarm<GroupNetworkBehaviour>,
    pub(crate) net_message_notifier: MessageReceiverHandle,
    pub(crate) net_broadcast_notifier: BroadcastReceiverHandle,
    pub(crate) ephemera_message_receiver: EphemeraMessagesReceiver,
}

impl SwarmNetwork {
    pub(crate) fn new(
        conf: Configuration,
        keypair: Arc<Libp2pKeypair>,
    ) -> (
        SwarmNetwork,
        EphemeraMessagesNotifier,
        MessagesReceiver,
        NetworkBroadcastReceiver,
    ) {
        let (
            net_broadcast_notify_receiver,
            net_broadcast_notifier,
            net_message_notify_receiver,
            net_message_notifier,
        ) = NetworkMessages::init();

        let (ephemera_message_notifier, ephemera_message_receiver) =
            EphemeraMessagesReceiver::init();

        let swarm = SwarmNetwork::create_swarm(conf.clone(), keypair);

        let network = SwarmNetwork {
            config: conf,
            swarm,
            net_message_notifier,
            net_broadcast_notifier,
            ephemera_message_receiver,
        };

        (
            network,
            ephemera_message_notifier,
            net_message_notify_receiver,
            net_broadcast_notify_receiver,
        )
    }
    //Message delivery and peer discovery
    fn create_swarm(
        conf: Configuration,
        keypair: Arc<Libp2pKeypair>,
    ) -> Swarm<GroupNetworkBehaviour> {
        let keypair: &Libp2pKeypair = keypair.borrow();
        let local_id = Libp2pPeerId::from(keypair.as_ref().public());
        let transport = create_transport(keypair.as_ref());
        let behaviour = create_behaviour(conf, keypair.as_ref());

        Swarm::with_tokio_executor(transport, behaviour, local_id)
    }

    pub(crate) async fn start(mut self) {
        let address = Multiaddr::from_str(self.config.node_config.address.as_str())
            .expect("Invalid multi-address");
        self.swarm.listen_on(address).unwrap();

        let consensus_msg_topic = Topic::new(&self.config.libp2p.consensus_msg_topic_name);
        let proposed_msg_topic = Topic::new(&self.config.libp2p.proposed_msg_topic_name);

        loop {
            select!(
                // Handle incoming messages from the network
                swarm_event = self.swarm.select_next_some() => {
                    self.handle_incoming_messages(
                        swarm_event,
                        &consensus_msg_topic,
                        &proposed_msg_topic).await;
                },
                // Handle messages from broadcast_protocol
                Some(cm) = self.ephemera_message_receiver.broadcast_sender_rcv.recv() => {
                     self.send_protocol_message(cm, &consensus_msg_topic,).await;
                }
                // Handle messages from broadcast_protocol
                Some(pm) = self.ephemera_message_receiver.message_sender_rcv.recv() => {
                    self.send_proposed_message(pm, &proposed_msg_topic,).await;
                }
            );
        }
    }

    #[allow(clippy::collapsible_match, clippy::single_match)]
    async fn handle_incoming_messages<E>(
        &mut self,
        swarm_event: SwarmEvent<GroupBehaviourEvent, E>,
        protocol_msg_topic: &Topic,
        signed_msg_topic: &Topic,
    ) {
        match swarm_event {
            SwarmEvent::Behaviour(b) => match b {
                GroupBehaviourEvent::Gossipsub(gs) => match gs {
                    GossipsubEvent::Message {
                        propagation_source,
                        message_id: _,
                        message,
                    } => {
                        if message.topic == (*protocol_msg_topic).clone().into() {
                            let msg: RbMsg =
                                serde_json::from_slice(&message.data[..]).unwrap();
                            log::trace!(
                                "Received protocol message {:?} from {}",
                                msg,
                                propagation_source
                            );

                            let nm = NetworkMessage::new(msg);
                            if let Err(err) = self.net_broadcast_notifier.send(nm).await {
                                log::error!(
                                    "Error sending message to broadcaster channel: {}",
                                    err
                                );
                            }
                        } else if message.topic == (*signed_msg_topic).clone().into() {
                            log::trace!("Received signed message from {}", propagation_source);

                            let msg = serde_json::from_slice(&message.data[..]).unwrap();
                            log::trace!("Received signed message: {:?}", msg);

                            if let Err(err) = self.net_message_notifier.send(msg).await {
                                log::error!("Error sending message to channel: {}", err);
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
            //Ignore other Swarm events for now
            _ => {}
        }
    }

    async fn send_protocol_message(&mut self, msg: RbMsg, topic: &Topic) {
        log::trace!("Sending protocol message: {}", msg.id);
        let vec = serde_json::to_vec(&msg).unwrap();
        if let Err(err) = self
            .swarm
            .behaviour_mut()
            .gossipsub
            .publish(topic.clone(), vec)
        {
            log::error!("Error publishing message: {}", err);
        }
    }

    async fn send_proposed_message(&mut self, msg: SignedMessage, topic: &Topic) {
        log::trace!("Sending proposed message: {}", msg.id);
        let vec = serde_json::to_vec(&msg).unwrap();
        if let Err(err) = self
            .swarm
            .behaviour_mut()
            .gossipsub
            .publish(topic.clone(), vec)
        {
            log::error!("Error publishing message: {}", err);
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

//Create combined behaviour.
//Gossipsub takes care of message delivery semantics
//Peer discovery takes care of locating peers
fn create_behaviour(conf: Configuration, local_key: &Keypair) -> GroupNetworkBehaviour {
    let consensus_topic = Topic::new(&conf.libp2p.consensus_msg_topic_name);
    let proposed_topic = Topic::new(&conf.libp2p.proposed_msg_topic_name);

    let mut gossipsub = create_gossipsub(local_key);
    gossipsub.subscribe(&consensus_topic).unwrap();
    gossipsub.subscribe(&proposed_topic).unwrap();

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
fn create_gossipsub(local_key: &Keypair) -> Gossipsub {
    let gossipsub_config = GossipsubConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(5))
        .validation_mode(ValidationMode::Strict)
        .build()
        .expect("Valid config");

    Gossipsub::new(
        MessageAuthenticity::Signed(local_key.clone()),
        gossipsub_config,
    )
    .expect("Correct configuration")
}

//Configure networking connection stack(Tcp, Noise, Yamux)
//Tcp protocol for networking
//Noise protocol for encryption
//Yamux protocol for multiplexing
fn create_transport(local_key: &Keypair) -> Boxed<(Libp2pPeerId, StreamMuxerBox)> {
    let transport = TokioTransport::new(TokioConfig::default().nodelay(true));
    let noise_keypair = noise::Keypair::<noise::X25519Spec>::new()
        .into_authentic(local_key)
        .unwrap();
    let xx_config = noise::NoiseConfig::xx(noise_keypair);
    transport
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(xx_config.into_authenticated())
        .multiplex(libp2p::core::upgrade::SelectUpgrade::new(
            YamuxConfig::default(),
            MplexConfig::default(),
        ))
        .timeout(Duration::from_secs(20))
        .boxed()
}
