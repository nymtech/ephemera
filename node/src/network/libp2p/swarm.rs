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

use crate::block::types::message::EphemeraMessage;
use crate::broadcast::RbMsg;
use crate::config::Configuration;
use crate::network::libp2p::messages_channel::{
    EphemeraNetworkCommunication, NetCommunicationReceiver, NetCommunicationSender,
};
use crate::network::libp2p::peer_discovery::StaticPeerDiscovery;
use crate::utilities::crypto::ed25519::Ed25519Keypair;

pub struct SwarmNetwork {
    config: Configuration,
    swarm: Swarm<GroupNetworkBehaviour>,
    from_ephemera_rcv: NetCommunicationReceiver,
    to_ephemera_tx: NetCommunicationSender,
}

impl SwarmNetwork {
    pub(crate) fn new(
        config: Configuration,
        keypair: Arc<Ed25519Keypair>,
    ) -> (
        SwarmNetwork,
        NetCommunicationReceiver,
        NetCommunicationSender,
    ) {
        let (from_ephemera_tx, from_ephemera_rcv) = EphemeraNetworkCommunication::init();
        let (to_ephemera_tx, to_ephemera_rcv) = EphemeraNetworkCommunication::init();

        let swarm = SwarmNetwork::create_swarm(config.clone(), keypair);

        let network = SwarmNetwork {
            config,
            swarm,
            from_ephemera_rcv,
            to_ephemera_tx,
        };

        (network, to_ephemera_rcv, from_ephemera_tx)
    }
    //Message delivery and peer discovery
    fn create_swarm(
        conf: Configuration,
        keypair: Arc<Ed25519Keypair>,
    ) -> Swarm<GroupNetworkBehaviour> {
        let keypair: &Ed25519Keypair = keypair.borrow();
        let local_id = Libp2pPeerId::from(keypair.as_ref().public());
        let transport = create_transport(keypair.as_ref());
        let behaviour = create_behaviour(conf, keypair.as_ref());

        Swarm::with_tokio_executor(transport, behaviour, local_id)
    }

    pub(crate) fn listen(&mut self) -> anyhow::Result<()> {
        let address = Multiaddr::from_str(self.config.node_config.address.as_str())
            .expect("Invalid multi-address");
        self.swarm.listen_on(address.clone())?;

        log::info!("Listening on {address:?}");
        Ok(())
    }

    pub(crate) async fn start(mut self) -> anyhow::Result<()> {
        let consensus_msg_topic = Topic::new(&self.config.libp2p.consensus_msg_topic_name);
        let ephemera_msg_topic = Topic::new(&self.config.libp2p.proposed_msg_topic_name);

        loop {
            select!(
                swarm_event = self.swarm.next() => {
                    match swarm_event{
                        Some(event) => {
                            if let Err(err) = self.handle_incoming_messages(event, &consensus_msg_topic, &ephemera_msg_topic).await{
                                log::error!("Error handling swarm event: {:?}", err);
                            }
                        }
                        None => {
                            anyhow::bail!("Swarm event channel closed")
                        }
                    }
                },
                cm_maybe = self.from_ephemera_rcv.ephemera_message_receiver.recv() => {
                    if let Some(cm) = cm_maybe {
                        self.send_ephemera_message(cm, &ephemera_msg_topic,).await;
                    }
                    else {
                        anyhow::bail!("ephemera_message_receiver channel closed")
                    }
                }
                pm_maybe = self.from_ephemera_rcv.protocol_msg_receiver.recv() => {
                    if let Some(pm) = pm_maybe {
                        self.send_protocol_message(pm, &consensus_msg_topic,).await;
                    }
                    else {
                        anyhow::bail!("protocol_msg_receiver channel closed")
                    }
                }
            );
        }
    }

    #[allow(clippy::collapsible_match, clippy::single_match)]
    async fn handle_incoming_messages<E>(
        &mut self,
        swarm_event: SwarmEvent<GroupBehaviourEvent, E>,
        protocol_msg_topic: &Topic,
        ephemera_msg_topic: &Topic,
    ) -> anyhow::Result<()> {
        match swarm_event {
            SwarmEvent::Behaviour(b) => match b {
                GroupBehaviourEvent::Gossipsub(gs) => match gs {
                    GossipsubEvent::Message {
                        propagation_source: _,
                        message_id: _,
                        message,
                    } => {
                        if message.topic == (*protocol_msg_topic).clone().into() {
                            self.to_ephemera_tx
                                .send_protocol_message_raw(message.data)
                                .await?;
                        } else if message.topic == (*ephemera_msg_topic).clone().into() {
                            self.to_ephemera_tx
                                .send_ephemera_message_raw(message.data)
                                .await?;
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
            //Ignore other Swarm events for now
            _ => {}
        }
        Ok(())
    }

    async fn send_protocol_message(&mut self, msg: RbMsg, topic: &Topic) {
        log::trace!("Sending protocol message: {}", msg.id);
        self.send_message(msg, topic).await;
    }

    async fn send_ephemera_message(&mut self, msg: EphemeraMessage, topic: &Topic) {
        log::trace!("Sending proposed message: {}", msg.id);
        self.send_message(msg, topic).await;
    }

    async fn send_message<T: serde::Serialize>(&mut self, msg: T, topic: &Topic) {
        match serde_json::to_vec(&msg) {
            Ok(vec) => {
                if let Err(err) = self
                    .swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(topic.clone(), vec)
                {
                    log::error!("Error publishing message: {}", err);
                }
            }
            Err(err) => {
                log::error!("Error serializing message: {}", err);
            }
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
