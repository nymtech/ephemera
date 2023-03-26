use std::str::FromStr;

use futures::StreamExt;
use libp2p::{
    gossipsub::{self, IdentTopic as Topic},
    Multiaddr, request_response, Swarm,
};
use libp2p::gossipsub::Event;
use libp2p::kad;
use libp2p::swarm::SwarmEvent;

use crate::block::types::message::EphemeraMessage;
use crate::broadcast::RbMsg;
use crate::config::{Libp2pConfig, NodeConfig};
use crate::core::builder::NodeInfo;
use crate::network::libp2p::behaviours::broadcast_messages::RbMsgResponse;
use crate::network::libp2p::behaviours::common_behaviour::{
    create_behaviour, create_transport, GroupBehaviourEvent, GroupNetworkBehaviour,
};
use crate::network::libp2p::discovery::rendezvous;
use crate::network::libp2p::ephemera_sender::{
    EphemeraEvent, EphemeraToNetwork, EphemeraToNetworkReceiver, EphemeraToNetworkSender,
};
use crate::network::libp2p::network_sender::{
    EphemeraNetworkCommunication, NetCommunicationReceiver, NetCommunicationSender, NetworkEvent,
};
use crate::network::PeerDiscovery;

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
                            if let Err(err) = self.handle_incoming_messages(event).await{
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
    ) -> anyhow::Result<()> {
        if let SwarmEvent::Behaviour(b) = swarm_event {
            if let Err(err) = self.process_group_behaviour_event(b).await {
                log::error!("Error handling behaviour event: {:?}", err);
            }
        } else if let Err(err) = self.process_other_swarm_events(swarm_event).await {
            log::error!("Error handling swarm event: {:?}", err);
        }
        Ok(())
    }

    async fn process_group_behaviour_event(
        &mut self,
        event: GroupBehaviourEvent,
    ) -> anyhow::Result<()> {
        match event {
            GroupBehaviourEvent::Gossipsub(gs) => {
                if let Err(err) = self.process_gossipsub_event(gs).await {
                    log::error!("Error processing gossipsub event: {:?}", err);
                }
            }
            GroupBehaviourEvent::RequestResponse(request_response) => {
                if let Err(err) = self.process_request_response(request_response).await {
                    log::error!("Error processing request response: {:?}", err);
                }
            }

            GroupBehaviourEvent::Rendezvous(event) => {
                if let Err(err) = self.process_rendezvous_event(event).await {
                    log::error!("Error processing rendezvous event: {:?}", err);
                }
            }
            GroupBehaviourEvent::Kademlia(ev) => {
                if let Err(err) = self.process_kad_event(ev).await {
                    log::error!("Error processing kademlia event: {:?}", err);
                }
            }
        }
        Ok(())
    }

    async fn process_gossipsub_event(&mut self, event: gossipsub::Event) -> anyhow::Result<()> {
        match event {
            gossipsub::Event::Message {
                propagation_source: _,
                message_id: _,
                message,
            } => {
                let msg: EphemeraMessage = serde_json::from_slice(&message.data[..])?;
                self.to_ephemera_tx
                    .send_network_event(NetworkEvent::EphemeraMessage(msg.into()))
                    .await?;
            }

            Event::Subscribed { peer_id, topic } => {
                log::debug!("Peer {peer_id:?} subscribed to topic {topic:?}");
            }
            Event::Unsubscribed { peer_id, topic } => {
                log::debug!("Peer {peer_id:?} unsubscribed from topic {topic:?}");
            }
            Event::GossipsubNotSupported { peer_id } => {
                log::debug!("Peer {peer_id:?} does not support gossipsub");
            }
        }
        Ok(())
    }

    async fn process_request_response(
        &mut self,
        event: request_response::Event<RbMsg, RbMsgResponse>,
    ) -> anyhow::Result<()> {
        match event {
            request_response::Event::Message { peer, message } => match message {
                request_response::Message::Request {
                    request_id,
                    request,
                    channel,
                } => {
                    log::debug!("Received request {request_id:?} from peer: {peer:?}",);
                    let rb_id = request.id.clone();
                    self.to_ephemera_tx
                        .send_network_event(NetworkEvent::ProtocolMessage(request.into()))
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
                log::error!(
                    "Outbound failure: {error:?}, peer:{peer:?}, request_id:{request_id:?}",
                );
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
        Ok(())
    }

    async fn process_rendezvous_event(&mut self, event: rendezvous::Event) -> anyhow::Result<()> {
        match event {
            rendezvous::Event::PeersUpdated => {
                log::info!(
                    "Peers updated: {:?}",
                    self.swarm.behaviour().rendezvous_behaviour.peer_ids_ref()
                );
                let new_peers = self.swarm.behaviour().rendezvous_behaviour.peers();

                let kademlia = &mut self.swarm.behaviour_mut().kademlia;

                for peer in new_peers {
                    kademlia.add_address(peer.peer_id.inner(), peer.address.clone());
                }
            }
        }
        Ok(())
    }

    async fn process_kad_event(&mut self, event: kad::KademliaEvent) -> anyhow::Result<()> {
        match event {
            kad::KademliaEvent::InboundRequest { request } => {
                log::debug!("Inbound request: {:?}", request);
            }
            kad::KademliaEvent::OutboundQueryProgressed {
                id,
                result,
                stats,
                step,
            } => {
                log::debug!(
                    "Outbound query progressed: id:{:?}, result:{:?}, stats:{:?}, step:{:?}",
                    id,
                    result,
                    stats,
                    step
                );
                match result {
                    kad::QueryResult::Bootstrap(bt) => {
                        log::debug!("Bootstrap: {:?}", bt);
                    }
                    kad::QueryResult::GetClosestPeers(gcp) => {
                        log::debug!("GetClosestPeers: {:?}", gcp);
                        //TODO: we need also to make sure that we hve enough peers
                        match gcp {
                            Ok(cp) => {
                                let previous_peer_ids = self
                                    .swarm
                                    .behaviour()
                                    .rendezvous_behaviour
                                    .previous_peer_ids();

                                let gossipsub = &mut self.swarm.behaviour_mut().gossipsub;
                                for peer_id in previous_peer_ids {
                                    //TODO: remove only peers that are not in the new list
                                    gossipsub.remove_explicit_peer(peer_id.inner());
                                }

                                for peer_id in cp.peers {
                                    gossipsub.add_explicit_peer(&peer_id);
                                }

                                let new_peer_ids =
                                    self.swarm.behaviour().rendezvous_behaviour.peer_ids();
                                self.to_ephemera_tx
                                    .send_network_event(NetworkEvent::PeersUpdated(new_peer_ids))
                                    .await?;
                            }
                            Err(err) => {
                                log::error!("Error getting closest peers: {:?}", err);
                            }
                        }
                    }
                    kad::QueryResult::GetProviders(gp) => {
                        log::debug!("GetProviders: {:?}", gp);
                    }
                    kad::QueryResult::StartProviding(sp) => {
                        log::debug!("StartProviding: {:?}", sp);
                    }
                    kad::QueryResult::RepublishProvider(rp) => {
                        log::debug!("RepublishProvider: {:?}", rp);
                    }
                    kad::QueryResult::GetRecord(gr) => {
                        log::debug!("GetRecord: {:?}", gr);
                    }
                    kad::QueryResult::PutRecord(pr) => {
                        log::debug!("PutRecord: {:?}", pr);
                    }
                    kad::QueryResult::RepublishRecord(rr) => {
                        log::debug!("RepublishRecord: {:?}", rr);
                    }
                }
            }
            kad::KademliaEvent::RoutingUpdated {
                peer,
                is_new_peer,
                addresses,
                bucket_range,
                old_peer,
            } => {
                log::debug!("Routing updated: peer:{:?}, is_new_peer:{:?}, addresses:{:?}, bucket_range:{:?}, old_peer:{:?}",
                    peer, is_new_peer, addresses, bucket_range, old_peer);

                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .get_closest_peers(libp2p::PeerId::random());
                log::debug!("NEIGHBOURS: {:?}", query_id);
            }
            kad::KademliaEvent::UnroutablePeer { peer } => {
                log::debug!("Unroutable peer: {:?}", peer);
            }
            kad::KademliaEvent::RoutablePeer { peer, address } => {
                log::debug!("Routable peer: {:?}, address: {:?}", peer, address);
            }
            kad::KademliaEvent::PendingRoutablePeer { peer, address } => {
                log::debug!("Pending routable peer: {:?}, address: {:?}", peer, address);
            }
        }
        Ok(())
    }

    async fn process_other_swarm_events<E>(
        &mut self,
        swarm_event: SwarmEvent<GroupBehaviourEvent, E>,
    ) -> anyhow::Result<()> {
        match swarm_event {
            SwarmEvent::ConnectionEstablished {
                peer_id,
                endpoint,
                num_established,
                concurrent_dial_errors,
                established_in,
            } => {
                log::debug!("Connection established: peer_id:{:?}, endpoint:{:?}, num_established:{:?}, concurrent_dial_errors:{:?}, established_in:{:?}", peer_id, endpoint, num_established, concurrent_dial_errors, established_in);
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                endpoint,
                num_established,
                cause: _,
            } => {
                log::debug!(
                    "Connection closed: peer_id:{:?}, endpoint:{:?}, num_established:{:?}",
                    peer_id,
                    endpoint,
                    num_established
                );
            }
            SwarmEvent::IncomingConnection {
                local_addr,
                send_back_addr,
            } => {
                log::debug!(
                    "Incoming connection: local_addr:{:?}, send_back_addr:{:?}",
                    local_addr,
                    send_back_addr
                );
            }
            SwarmEvent::IncomingConnectionError {
                local_addr,
                send_back_addr,
                error,
            } => {
                log::debug!(
                    "Incoming connection error: local_addr:{:?}, send_back_addr:{:?}, error:{:?}",
                    local_addr,
                    send_back_addr,
                    error
                );
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error } => {
                log::debug!(
                    "Outgoing connection error: peer_id:{:?}, error:{:?}",
                    peer_id,
                    error
                );
            }
            SwarmEvent::BannedPeer { peer_id, endpoint } => {
                log::debug!(
                    "Banned peer: peer_id:{:?}, endpoint:{:?}",
                    peer_id,
                    endpoint
                );
            }
            SwarmEvent::NewListenAddr {
                listener_id,
                address,
            } => {
                log::debug!(
                    "New listen address: listener_id:{:?}, address:{:?}",
                    listener_id,
                    address
                );
            }
            SwarmEvent::ExpiredListenAddr {
                listener_id,
                address,
            } => {
                log::debug!(
                    "Expired listen address: listener_id:{:?}, address:{:?}",
                    listener_id,
                    address
                );
            }
            SwarmEvent::ListenerClosed {
                listener_id,
                addresses,
                reason,
            } => {
                log::debug!(
                    "Listener closed: listener_id:{:?}, addresses:{:?}, reason:{:?}",
                    listener_id,
                    addresses,
                    reason
                );
            }
            SwarmEvent::ListenerError { listener_id, error } => {
                log::debug!(
                    "Listener error: listener_id:{:?}, error:{:?}",
                    listener_id,
                    error
                );
            }
            SwarmEvent::Dialing(peer_id) => {
                log::debug!("Dialing: {peer_id:?}",);
            }

            SwarmEvent::Behaviour(_) => {
                log::error!("Unexpected behaviour event");
            }
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
