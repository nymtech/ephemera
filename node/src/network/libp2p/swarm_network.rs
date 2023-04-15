use std::str::FromStr;

use futures::StreamExt;
use libp2p::{
    gossipsub, gossipsub::IdentTopic as Topic, kad, request_response, swarm::SwarmEvent, Multiaddr,
    Swarm,
};
use tokio::task::JoinHandle;

use crate::{
    block::types::message::EphemeraMessage,
    broadcast::RbMsg,
    codec::Encode,
    core::builder::NodeInfo,
    network::{
        discovery::PeerDiscovery,
        libp2p::{
            behaviours::peer_discovery,
            behaviours::{
                broadcast_messages::RbMsgResponse,
                common_behaviour::{
                    create_behaviour, create_transport, GroupBehaviourEvent, GroupNetworkBehaviour,
                },
            },
            ephemera_sender::{
                EphemeraEvent, EphemeraToNetwork, EphemeraToNetworkReceiver,
                EphemeraToNetworkSender,
            },
            network_sender::{
                EphemeraNetworkCommunication, NetCommunicationReceiver, NetCommunicationSender,
                NetworkEvent, TopologyEvent,
                TopologyEvent::{LocalPeerRemoved, NotEnoughPeers},
            },
        },
    },
};

pub struct SwarmNetwork<P: PeerDiscovery + 'static> {
    node_info: NodeInfo,
    swarm: Swarm<GroupNetworkBehaviour<P>>,
    from_ephemera_rcv: EphemeraToNetworkReceiver,
    to_ephemera_tx: NetCommunicationSender,
    ephemera_msg_topic: Topic,
}

impl<P: PeerDiscovery> SwarmNetwork<P> {
    pub(crate) fn new(
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
        let peer_id = node_info.peer_id;
        let ephemera_msg_topic =
            Topic::new(&node_info.initial_config.libp2p.ephemera_msg_topic_name);

        let transport = create_transport(local_key.clone());
        let behaviour = create_behaviour(local_key, ephemera_msg_topic.clone(), peer_discovery);
        let swarm = Swarm::with_tokio_executor(transport, behaviour, peer_id.into());

        let network = SwarmNetwork {
            node_info,
            swarm,
            from_ephemera_rcv,
            to_ephemera_tx,
            ephemera_msg_topic,
        };

        (network, to_ephemera_rcv, from_ephemera_tx)
    }

    pub(crate) fn listen(&mut self) -> anyhow::Result<()> {
        let address =
            Multiaddr::from_str(&self.node_info.protocol_address()).expect("Invalid multi-address");
        self.swarm.listen_on(address.clone())?;

        log::info!("Listening on {address:?}");
        Ok(())
    }

    pub(crate) async fn start(mut self) -> anyhow::Result<()> {
        // Spawn rendezvous behaviour outside of swarm event loop.
        // It would look better if it were integrated into libp2p architecture.
        // Maybe some good ideas will come up in the future.
        let mut rendezvous_handle = self.start_peer_discovery().await?;

        loop {
            tokio::select! {
                swarm_event = self.swarm.next() => {
                    match swarm_event{
                        Some(event) => {
                            if let Err(err) = self.handle_incoming_messages(event).await{
                                log::error!("Error handling swarm event: {:?}", err);
                            }
                        }
                        None => {
                            anyhow::bail!("Swarm event channel closed");
                        }
                    }
                },
                Some(event) = self.from_ephemera_rcv.net_event_rcv.recv() => {
                    self.process_ephemera_events(event).await;
                }
                _ = &mut rendezvous_handle => {
                    log::info!("Rendezvous behaviour finished");
                    return Ok(());
                }
            }
        }
    }

    async fn start_peer_discovery(&mut self) -> anyhow::Result<JoinHandle<()>> {
        self.swarm
            .behaviour_mut()
            .rendezvous_behaviour
            .spawn_peer_discovery()
            .await
    }

    async fn process_ephemera_events(&mut self, event: EphemeraEvent) {
        match event {
            EphemeraEvent::EphemeraMessage(em) => {
                self.send_ephemera_message(*em).await;
            }
            EphemeraEvent::ProtocolMessage(pm) => {
                self.send_broadcast_message(*pm).await;
            }
            EphemeraEvent::StoreInDht { key, value } => {
                let record = kad::Record::new(key, value);
                //TODO: review quorum size
                let quorum = kad::Quorum::One;
                match self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .put_record(record, quorum)
                {
                    Ok(ok) => {
                        log::debug!("StoreDht: {:?}", ok);
                    }
                    Err(err) => {
                        log::error!("StoreDht: {:?}", err);
                    }
                }
            }
            EphemeraEvent::QueryDht { key } => {
                let kad_key = kad::record::Key::new::<Vec<u8>>(key.as_ref());
                let query_id = self.swarm.behaviour_mut().kademlia.get_record(kad_key);
                log::trace!("QueryDht: {:?}", query_id);
            }
        }
    }

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

            GroupBehaviourEvent::PeerDiscovery(event) => {
                if let Err(err) = self.process_peer_discovery_event(event).await {
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

            gossipsub::Event::Subscribed { peer_id, topic } => {
                log::trace!("Peer {peer_id:?} subscribed to topic {topic:?}");
            }
            gossipsub::Event::Unsubscribed { peer_id, topic } => {
                log::trace!("Peer {peer_id:?} unsubscribed from topic {topic:?}");
            }
            gossipsub::Event::GossipsubNotSupported { peer_id } => {
                log::trace!("Peer {peer_id:?} does not support gossipsub");
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
                    request_id: _,
                    request,
                    channel,
                } => {
                    let rb_id = request.id.clone();
                    log::debug!("Received request {:?}", request.short_fmt());
                    self.to_ephemera_tx
                        .send_network_event(NetworkEvent::BroadcastMessage(request.into()))
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
                    log::trace!("Received response {response:?} from peer: {peer:?}, request_id: {request_id:?}",);
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
                log::trace!("Response sent to peer: {peer:?}, {request_id:?}",);
            }
        }
        Ok(())
    }

    async fn process_peer_discovery_event(
        &mut self,
        event: peer_discovery::behaviour::Event,
    ) -> anyhow::Result<()> {
        match event {
            peer_discovery::behaviour::Event::PeersUpdated => {
                log::info!(
                    "Peers updated: {:?}",
                    self.swarm.behaviour().rendezvous_behaviour.peer_ids()
                );
                let new_peers = self.swarm.behaviour().rendezvous_behaviour.peers();
                log::info!("New peers: {:?}", new_peers);

                let local_peer_id = *self.swarm.local_peer_id();
                let kademlia = &mut self.swarm.behaviour_mut().kademlia;

                for peer in new_peers {
                    if *peer.peer_id.inner() == local_peer_id {
                        continue;
                    }
                    kademlia.add_address(peer.peer_id.inner(), peer.address.inner());
                }

                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .get_closest_peers(libp2p::PeerId::random());
                log::debug!("Neighbours: {:?}", query_id);
            }
            peer_discovery::behaviour::Event::PeerUpdatePending => {
                log::info!("Peer update pending");
            }
            peer_discovery::behaviour::Event::LocalRemoved => {
                //TODO: should pause all network block and message activities
                let update = NetworkEvent::TopologyUpdate(LocalPeerRemoved);
                self.to_ephemera_tx.send_network_event(update).await?;
            }
            peer_discovery::behaviour::Event::NotEnoughPeers => {
                //TODO: should pause all network block and message activities
                let update = NetworkEvent::TopologyUpdate(NotEnoughPeers);
                self.to_ephemera_tx.send_network_event(update).await?;
            }
        }
        Ok(())
    }

    async fn process_kad_event(&mut self, event: kad::KademliaEvent) -> anyhow::Result<()> {
        match event {
            kad::KademliaEvent::InboundRequest { request } => {
                log::trace!("Inbound request: {:?}", request);
            }
            kad::KademliaEvent::OutboundQueryProgressed {
                id,
                result,
                stats,
                step,
            } => {
                log::trace!(
                    "Outbound query progressed: id:{:?}, result:{:?}, stats:{:?}, step:{:?}",
                    id,
                    result,
                    stats,
                    step
                );
                match result {
                    kad::QueryResult::Bootstrap(bt) => {
                        log::trace!("Bootstrap: {:?}", bt);
                    }
                    kad::QueryResult::GetClosestPeers(gcp) => {
                        log::debug!("GetClosestPeers: {:?}", gcp);
                        //TODO: we need also to make sure that we have enough peers
                        // (Repeat if not enough, may need to wait network to stabilize)

                        //TODO: kad seems to get multiple responses for single query
                        match gcp {
                            Ok(cp) => {
                                if cp.peers.is_empty() {
                                    return Ok(());
                                }
                                let previous_peer_ids = self
                                    .swarm
                                    .behaviour()
                                    .rendezvous_behaviour
                                    .previous_peer_ids();

                                let gossipsub = &mut self.swarm.behaviour_mut().gossipsub;
                                for peer_id in previous_peer_ids {
                                    gossipsub.remove_explicit_peer(peer_id.inner());
                                }

                                for peer_id in cp.peers {
                                    gossipsub.add_explicit_peer(&peer_id);
                                }

                                let new_peer_ids =
                                    self.swarm.behaviour().rendezvous_behaviour.peer_ids();

                                let topology_update = NetworkEvent::TopologyUpdate(
                                    TopologyEvent::PeersUpdated(new_peer_ids),
                                );
                                self.to_ephemera_tx
                                    .send_network_event(topology_update)
                                    .await?;
                            }
                            Err(err) => {
                                log::error!("Error getting closest peers: {:?}", err);
                            }
                        }
                    }
                    kad::QueryResult::GetProviders(gp) => {
                        log::trace!("GetProviders: {:?}", gp);
                    }
                    kad::QueryResult::StartProviding(sp) => {
                        log::trace!("StartProviding: {:?}", sp);
                    }
                    kad::QueryResult::RepublishProvider(rp) => {
                        log::trace!("RepublishProvider: {:?}", rp);
                    }
                    kad::QueryResult::GetRecord(get_res) => {
                        log::trace!("GetRecord: {:?}", get_res);
                        match get_res {
                            Ok(ok) => {
                                log::trace!("GetRecordOk: {:?}", ok);
                                match ok {
                                    kad::GetRecordOk::FoundRecord(fr) => {
                                        log::debug!("FoundRecord: {:?}", fr);
                                        let record = fr.record;
                                        let event = NetworkEvent::QueryDhtResponse {
                                            key: record.key.to_vec(),
                                            value: record.value,
                                        };
                                        self.to_ephemera_tx.send_network_event(event).await?;
                                    }
                                    kad::GetRecordOk::FinishedWithNoAdditionalRecord { .. } => {
                                        log::trace!("FinishedWithNoAdditionalRecord");
                                    }
                                }
                            }
                            Err(err) => {
                                log::trace!("Not getting record: {:?}", err);
                            }
                        }
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
                peer: peer_id,
                is_new_peer: _,
                addresses,
                bucket_range: _,
                old_peer: _,
            } => {
                log::debug!("Routing updated: peer:{peer_id}, addresses:{addresses:?}",);
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

    //For now just logging
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
                log::trace!("Connection established: peer_id:{:?}, endpoint:{:?}, num_established:{:?}, concurrent_dial_errors:{:?}, established_in:{:?}", peer_id, endpoint, num_established, concurrent_dial_errors, established_in);
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                endpoint,
                num_established,
                cause: _,
            } => {
                log::trace!(
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
                log::trace!(
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
                log::trace!(
                    "Incoming connection error: local_addr:{:?}, send_back_addr:{:?}, error:{:?}",
                    local_addr,
                    send_back_addr,
                    error
                );
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error } => {
                log::trace!(
                    "Outgoing connection error: peer_id:{:?}, error:{:?}",
                    peer_id,
                    error
                );
            }
            SwarmEvent::BannedPeer { peer_id, endpoint } => {
                log::trace!(
                    "Banned peer: peer_id:{:?}, endpoint:{:?}",
                    peer_id,
                    endpoint
                );
            }
            SwarmEvent::NewListenAddr {
                listener_id,
                address,
            } => {
                log::trace!(
                    "New listen address: listener_id:{:?}, address:{:?}",
                    listener_id,
                    address
                );
            }
            SwarmEvent::ExpiredListenAddr {
                listener_id,
                address,
            } => {
                log::trace!(
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
                log::trace!(
                    "Listener closed: listener_id:{:?}, addresses:{:?}, reason:{:?}",
                    listener_id,
                    addresses,
                    reason
                );
            }
            SwarmEvent::ListenerError { listener_id, error } => {
                log::trace!(
                    "Listener error: listener_id:{:?}, error:{:?}",
                    listener_id,
                    error
                );
            }
            SwarmEvent::Dialing(peer_id) => {
                log::trace!("Dialing: {peer_id:?}",);
            }

            SwarmEvent::Behaviour(_) => {
                log::trace!("Unexpected behaviour event");
            }
        }
        Ok(())
    }

    async fn send_broadcast_message(&mut self, msg: RbMsg) {
        log::debug!(
            "Sending broadcast message: {:?} to all peers",
            msg.short_fmt()
        );
        log::trace!("Sending broadcast message: {:?}", msg);

        for peer in self.swarm.behaviour().rendezvous_behaviour.peer_ids() {
            log::trace!("Sending broadcast message: {:?} to peer: {peer:?}", msg.id,);
            if peer.inner() == self.swarm.local_peer_id() {
                continue;
            }
            self.swarm
                .behaviour_mut()
                .request_response
                .send_request(peer.inner(), msg.clone());
        }
    }

    async fn send_ephemera_message(&mut self, msg: EphemeraMessage) {
        log::trace!("Sending Ephemera message: {:?}", msg);
        match msg.encode() {
            Ok(vec) => {
                let topic = self.ephemera_msg_topic.clone();
                if let Err(err) = self.swarm.behaviour_mut().gossipsub.publish(topic, vec) {
                    log::error!("Error publishing message: {}", err);
                }
            }
            Err(err) => {
                log::error!("Error serializing message: {}", err);
            }
        }
    }
}
