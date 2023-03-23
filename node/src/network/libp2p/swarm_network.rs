use crate::block::types::message::EphemeraMessage;
use crate::broadcast::RbMsg;
use crate::config::{Libp2pConfig, NodeConfig};
use crate::core::builder::NodeInfo;
use crate::network::libp2p::behaviours::messages::RbMsgResponse;
use crate::network::libp2p::discovery::rendezvous;
use crate::network::libp2p::ephemera_behaviour::{
    create_behaviour, create_transport, GroupBehaviourEvent, GroupNetworkBehaviour,
};
use crate::network::libp2p::ephemera_sender::{
    EphemeraEvent, EphemeraToNetwork, EphemeraToNetworkReceiver, EphemeraToNetworkSender,
};
use crate::network::libp2p::network_sender::{
    EphemeraNetworkCommunication, NetCommunicationReceiver, NetCommunicationSender, NetworkEvent,
};
use crate::network::PeerDiscovery;
use futures::StreamExt;
use libp2p::gossipsub::IdentTopic as Topic;
use libp2p::swarm::SwarmEvent;
use libp2p::{gossipsub, request_response, Multiaddr, Swarm};
use std::str::FromStr;

pub struct SwarmNetwork<P: PeerDiscovery + 'static> {
    libp2p_conf: Libp2pConfig,
    node_conf: NodeConfig,
    swarm: Swarm<GroupNetworkBehaviour<P>>,
    from_ephemera_rcv: EphemeraToNetworkReceiver,
    to_ephemera_tx: NetCommunicationSender,
}

impl<P: PeerDiscovery> SwarmNetwork<P> {
    pub(crate) fn new(
        libp2p_conf: Libp2pConfig,
        node_conf: NodeConfig,
        node_info: NodeInfo,
        peer_discovery: P,
    ) -> (
        SwarmNetwork<P>,
        NetCommunicationReceiver,
        EphemeraToNetworkSender,
    ) {
        let (from_ephemera_tx, from_ephemera_rcv) = EphemeraToNetwork::init();
        let (to_ephemera_tx, to_ephemera_rcv) = EphemeraNetworkCommunication::init();

        let local_key = node_info.keypair.clone();
        let peer_id = node_info.peer_id.0;

        let transport = create_transport(local_key.clone());
        let behaviour = create_behaviour(&libp2p_conf, local_key, peer_discovery);
        let swarm = Swarm::with_tokio_executor(transport, behaviour, peer_id);

        let network = SwarmNetwork {
            libp2p_conf,
            node_conf,
            swarm,
            from_ephemera_rcv,
            to_ephemera_tx,
        };

        (network, to_ephemera_rcv, from_ephemera_tx)
    }

    pub(crate) fn listen(&mut self) -> anyhow::Result<()> {
        let address =
            Multiaddr::from_str(self.node_conf.address.as_str()).expect("Invalid multi-address");
        self.swarm.listen_on(address.clone())?;

        log::info!("Listening on {address:?}");
        Ok(())
    }

    pub(crate) async fn start(mut self) -> anyhow::Result<()> {
        let ephemera_msg_topic = Topic::new(&self.libp2p_conf.ephemera_msg_topic_name);

        // Spawn rendezvous behaviour outside of swarm event loop.
        // It would look better if it were integrated into libp2p architecture.
        // Maybe some good ideas will come up in the future.
        self.swarm
            .behaviour_mut()
            .rendezvous_behaviour
            .spawn()
            .await;

        loop {
            tokio::select!(
                swarm_event = self.swarm.next() => {
                    match swarm_event{
                        Some(event) => {
                            if let Err(err) = self.handle_incoming_messages(event, &ephemera_msg_topic).await{
                                log::error!("Error handling swarm event: {:?}", err);
                            }
                        }
                        None => {
                            anyhow::bail!("Swarm event channel closed")
                        }
                    }
                },
                Some(event) = self.from_ephemera_rcv.net_event_rcv.recv() => {
                    self.process_ephemera_events(event).await;
                }
            );
        }
    }

    async fn process_ephemera_events(&mut self, event: EphemeraEvent) {
        //FIXME:keep it around instead of create it every time
        let ephemera_msg_topic = Topic::new(&self.libp2p_conf.ephemera_msg_topic_name);
        match event {
            EphemeraEvent::EphemeraMessage(em) => {
                self.send_ephemera_message(*em, &ephemera_msg_topic).await;
            }
            EphemeraEvent::ProtocolMessage(pm) => {
                self.send_protocol_message(*pm).await;
            }
        }
    }

    #[allow(clippy::collapsible_match, clippy::single_match)]
    async fn handle_incoming_messages<E>(
        &mut self,
        swarm_event: SwarmEvent<GroupBehaviourEvent, E>,
        ephemera_msg_topic: &Topic,
    ) -> anyhow::Result<()> {
        match swarm_event {
            SwarmEvent::Behaviour(b) => match b {
                GroupBehaviourEvent::Gossipsub(gs) => match gs {
                    gossipsub::Event::Message {
                        propagation_source: _,
                        message_id: _,
                        message,
                    } => {
                        if message.topic == (*ephemera_msg_topic).clone().into() {
                            let msg: EphemeraMessage = serde_json::from_slice(&message.data[..])?;
                            self.to_ephemera_tx
                                .send_network_event(NetworkEvent::EphemeraMessage(msg.into()))
                                .await?;
                        }
                    }
                    _ => {}
                },
                GroupBehaviourEvent::RequestResponse(request_response) => {
                    match request_response {
                        request_response::Event::Message { peer, message } => match message {
                            request_response::Message::Request {
                                request_id,
                                request,
                                channel,
                            } => {
                                log::debug!("Received request {request_id:?} from peer: {peer:?}",);
                                let rb_id = request.id.clone();
                                self.to_ephemera_tx
                                    .send_network_event(NetworkEvent::ProtocolMessage(
                                        request.into(),
                                    ))
                                    .await?;
                                if let Err(err) = self
                                    .swarm
                                    .behaviour_mut()
                                    .request_response
                                    .send_response(channel, RbMsgResponse::new(rb_id))
                                {
                                    log::error!("Error sending response: {:?}", err);
                                }
                            }
                            request_response::Message::Response {
                                request_id,
                                response,
                            } => {
                                log::debug!("Received response {response:?} from peer: {peer:?}, request_id: {request_id:?}",);
                            }
                        },
                        request_response::Event::OutboundFailure {
                            peer,
                            request_id,
                            error,
                        } => {
                            log::error!("Outbound failure: {error:?}, peer:{peer:?}, request_id:{request_id:?}",);
                        }
                        request_response::Event::InboundFailure {
                            peer,
                            request_id,
                            error,
                        } => {
                            log::error!("Inbound failure: {error:?}, peer:{peer:?}, request_id:{request_id:?}",);
                        }
                        request_response::Event::ResponseSent { peer, request_id } => {
                            log::debug!("Response sent to peer: {peer:?}, {request_id:?}",);
                        }
                    }
                }

                GroupBehaviourEvent::Rendezvous(event) => match event {
                    rendezvous::Event::PeersUpdated => {
                        log::info!(
                            "Peers updated: {:?}",
                            self.swarm.behaviour_mut().rendezvous_behaviour.peer_ids()
                        );
                        let new_peer_ids =
                            self.swarm.behaviour_mut().rendezvous_behaviour.peer_ids();
                        let previous_peer_ids = self
                            .swarm
                            .behaviour_mut()
                            .rendezvous_behaviour
                            .previous_peer_ids();
                        let gossipsub = &mut self.swarm.behaviour_mut().gossipsub;

                        for peer_id in previous_peer_ids {
                            gossipsub.remove_explicit_peer(peer_id.inner());
                        }

                        for peer_id in &new_peer_ids {
                            gossipsub.add_explicit_peer(peer_id.inner());
                        }
                        self.to_ephemera_tx
                            .send_network_event(NetworkEvent::PeersUpdated(new_peer_ids))
                            .await?;
                    }
                },
            },
            //Ignore other Swarm events for now
            _ => {}
        }
        Ok(())
    }

    async fn send_protocol_message(&mut self, msg: RbMsg) {
        log::debug!("Sending Block message: {}", msg.id);
        for peer in self.swarm.behaviour().rendezvous_behaviour.peer_ids() {
            log::trace!("Sending Block message to peer: {:?}", peer);
            self.swarm
                .behaviour_mut()
                .request_response
                .send_request(&peer.into(), msg.clone());
        }
    }

    async fn send_ephemera_message(&mut self, msg: EphemeraMessage, topic: &Topic) {
        log::debug!("Sending Ephemera message: {}", msg.id);
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
