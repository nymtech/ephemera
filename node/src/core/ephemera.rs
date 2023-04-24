use std::collections::HashSet;
use std::sync::Arc;

use anyhow::anyhow;
use futures_util::StreamExt;
use log::{debug, error, info, trace};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::api::application::CheckBlockResult;
use crate::broadcast::bracha::broadcaster::BroadcastResponse;
use crate::broadcast::group::BroadcastGroup;
use crate::network::libp2p::network_sender::GroupChangeEvent;
use crate::network::PeerId;
use crate::{
    api::{application::Application, ApiListener},
    block::{manager::BlockManager, types::block::Block},
    broadcast::{bracha::broadcaster::Broadcaster, RbMsg},
    core::{
        api_cmd::ApiCmdProcessor,
        builder::{EphemeraHandle, NodeInfo},
        shutdown::ShutdownManager,
    },
    network::libp2p::{
        ephemera_sender::{EphemeraEvent, EphemeraToNetworkSender},
        network_sender::{NetCommunicationReceiver, NetworkEvent},
    },
    storage::EphemeraDatabase,
    utilities::crypto::Certificate,
    websocket::ws_manager::WsMessageBroadcaster,
};

//Just a placeholder now
#[derive(Error, Debug)]
enum EphemeraCoreError {
    #[error("EphemeraCoreError::GeneralError: {0}")]
    GeneralError(#[from] anyhow::Error),
}

type Result<T> = std::result::Result<T, EphemeraCoreError>;

pub struct Ephemera<A: Application> {
    /// Node info
    pub(crate) node_info: NodeInfo,

    /// Block manager responsibility includes:
    /// - Block production and signing
    /// - Block verification for externally received blocks
    /// - Message verification sent by clients and gossiped other nodes
    pub(crate) block_manager: BlockManager,

    /// Broadcaster is making sure that blocks are deterministically agreed by all nodes.
    pub(crate) broadcaster: Broadcaster,

    /// A component which receives messages from network.
    pub(crate) from_network: NetCommunicationReceiver,

    /// A component which sends messages to network.
    pub(crate) to_network: EphemeraToNetworkSender,

    /// A component which keeps track of broadcast group over time.
    pub(crate) broadcast_group: BroadcastGroup,

    /// A component which has mutable access to database.
    pub(crate) storage: Arc<Mutex<Box<dyn EphemeraDatabase>>>,

    /// A component which broadcasts messages to websocket clients.
    pub(crate) ws_message_broadcast: WsMessageBroadcaster,

    /// A component which listens API requests.
    pub(crate) api_listener: ApiListener,

    /// A component which processes API requests.
    pub(crate) api_cmd_processor: ApiCmdProcessor,

    /// An implementation of Application trait. Provides callbacks to broadcast.
    pub(crate) application: Arc<A>,

    ///Interface to external Rust code
    pub(crate) ephemera_handle: EphemeraHandle,

    /// A component which handles shutdown.
    pub(crate) shutdown_manager: Option<ShutdownManager>,
}

impl<A: Application> Ephemera<A> {
    ///Provides external api for Rust code to interact with ephemera node.
    pub fn handle(&self) -> EphemeraHandle {
        self.ephemera_handle.clone()
    }

    /// Main loop of ephemera node.
    /// 1. Block manager generates new blocks.
    /// 2. Receive messages from network.
    /// 3. Receive blocks from network.
    /// 4. Receive http api request
    /// 5. Receive rust api request
    /// 6. Publish(gossip) messages to network
    /// 7. Publish blocks to network
    pub async fn run(mut self) {
        let mut shutdown_manager = self
            .shutdown_manager
            .take()
            .expect("Shutdown manager not set");

        info!("Starting ephemera main loop");
        loop {
            tokio::select! {
                // GENERATING NEW BLOCKS
                Some((new_block, certificate)) = self.block_manager.next() => {
                    if let Err(err) = self.process_new_local_block(new_block, certificate).await{
                        error!("Error processing new block: {:?}", err);
                    }
                }

                // PROCESSING NETWORK EVENTS
                Some(net_event) = self.from_network.net_event_rcv.recv() => {
                    if let Err(err) = self.process_network_event(net_event).await{
                        error!("Error processing network event: {:?}", err);
                    }
                }

                //PROCESSING EXTERNAL API REQUESTS
                api = self.api_listener.messages_rcv.recv() => {
                    match api {
                        Some(api_msg) => {
                            if let Err(err) = ApiCmdProcessor::process_api_requests(&mut self, api_msg).await{
                                error!("Error processing api request: {:?}", err);
                            }
                        }
                        None => {
                            error!("Error: Api listener channel closed");
                            //TODO: handle shutdown
                        }
                    }
                }

                //PROCESSING SHUTDOWN REQUEST
                _ = shutdown_manager.external_shutdown.recv() => {
                    info!("Shutting down ephemera");
                    shutdown_manager.stop().await;
                    break;
                }
            }
        }
    }

    async fn process_network_event(&mut self, net_event: NetworkEvent) -> Result<()> {
        trace!("New network event: {:?}", net_event);

        match net_event {
            NetworkEvent::EphemeraMessage(em) => {
                let api_msg = (*em.clone()).into();
                debug!("New ephemera message from network: {:?}", api_msg);

                //Only Application checks if messages are valid(possibly message origin).
                //For messages we don't check if sender belongs to group.

                // Ask application to decide if we should accept this message.
                match self.application.check_tx(api_msg) {
                    Ok(true) => {
                        debug!("Application accepted message: {:?}", em);

                        // Send to BlockManager to store in mempool.
                        if let Err(err) = self.block_manager.on_new_message(*em) {
                            error!("Error sending signed message to block manager: {:?}", err);
                        }
                    }
                    Ok(false) => {
                        debug!("Application rejected message: {:?}", em);
                    }
                    Err(err) => {
                        error!("Application check_tx failed: {:?}", err);
                    }
                }
            }
            NetworkEvent::BroadcastMessage(rb_msg) => {
                if let Err(err) = self.process_block_from_network(*rb_msg).await {
                    error!("Error processing block from network: {:?}", err);
                }
            }
            NetworkEvent::GroupUpdate(event) => {
                self.process_group_update(event);
            }
            NetworkEvent::QueryDhtResponse { key, value } => {
                debug!("New dht query response: {:?}", key);
                match self.api_cmd_processor.dht_query_cache.pop(&key) {
                    Some(reply) => {
                        let response = Ok(Some((key, value)));
                        if let Err(err) = reply.send(response) {
                            error!("Error sending dht query response: {:?}", err);
                        }
                    }
                    None => {
                        error!(
                            "Error: No dht query cache found for key: {:?}",
                            String::from_utf8(key)
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn process_group_update(&mut self, event: GroupChangeEvent) {
        match event {
            GroupChangeEvent::PeersUpdated(peers) => {
                debug!("New peers: {:?}", peers);
                self.broadcaster.group_updated(peers.len());
                self.broadcast_group.add_snapshot(peers);
                self.block_manager.start();
            }
            GroupChangeEvent::LocalPeerRemoved(peers) | GroupChangeEvent::NotEnoughPeers(peers) => {
                info!("Group update: Local peer removed or not enough peers");
                self.broadcaster.group_updated(0);
                self.broadcast_group.add_snapshot(peers);
                self.block_manager.stop();
            }
        }
    }

    async fn process_new_local_block(
        &mut self,
        new_block: Block,
        certificate: Certificate,
    ) -> Result<()> {
        debug!("New block from block manager: {:?}", new_block.get_hash());

        let hash = new_block.header.hash;
        let block_creator = &self.node_info.peer_id;
        let sender = &self.node_info.peer_id;

        // Check if block matches group membership.
        if !self
            .broadcast_group
            .check_membership(hash, block_creator, sender)
        {
            debug!("Membership check rejected block: {:?}", new_block);
            return Ok(());
        }

        //Ephemera ABCI
        match self.application.check_block(&new_block.clone().into()) {
            Ok(response) => match response {
                CheckBlockResult::Accept => {
                    debug!("Application accepted block: {hash:?}",);
                }
                CheckBlockResult::Reject => {
                    debug!("Application rejected block: {hash:?}",);
                    return Ok(());
                }
                CheckBlockResult::RejectAndRemoveMessages(messages_to_remove) => {
                    debug!("Application rejected block: {:?}", messages_to_remove);
                    self.block_manager
                        .on_application_rejected_block(messages_to_remove)
                        .map_err(|err| {
                            anyhow!("Error rejecting block from block manager: {:?}", err)
                        })?;
                }
            },
            Err(err) => {
                return Err(anyhow!("Application check_block failed: {:?}", err).into());
            }
        }

        //Block manager generated new block that nobody hasn't seen yet.
        //We start reliable broadcaster protocol to broadcaster it to other nodes.
        match self.broadcaster.new_broadcast(new_block).await {
            Ok(resp) => {
                if let BroadcastResponse::Broadcast(msg) = resp {
                    trace!("Broadcasting new block: {:?}", msg);

                    let rb_msg = RbMsg::new(msg, certificate);
                    self.to_network
                        .send_ephemera_event(EphemeraEvent::ProtocolMessage(rb_msg.into()))
                        .await?;
                }
            }
            Err(err) => {
                error!("Error starting new broadcast: {:?}", err);
            }
        }
        Ok(())
    }

    //TODO: should we accept more blocks from peers after its committed?
    async fn process_block_from_network(&mut self, msg: RbMsg) -> Result<()> {
        let msg_id = msg.id.clone();
        let block = msg.block();
        let block_creator = &block.header.creator;
        let sender = &msg.original_sender;
        let hash = block.header.hash;
        let certificate = msg.certificate.clone();

        debug!(
            "New broadcast message from network: {:?} for block: {hash:?} from peer: {sender:?}",
            msg.id
        );
        trace!("New broadcast message from network: {:?}", msg);

        if !self
            .broadcast_group
            .check_membership(hash, block_creator, sender)
        {
            return Err(anyhow!("Block doesn't match broacast group").into());
        }

        if let Err(err) = self.block_manager.on_block(sender, block, &certificate) {
            return Err(anyhow!("Error sending block to block manager: {:?}", err).into());
        }
        match self.broadcaster.handle(msg.into()).await {
            Ok(resp) => {
                match resp {
                    BroadcastResponse::Broadcast(msg) => {
                        trace!("Broadcasting block to network: {:?}", msg);

                        match self.block_manager.sign_block(&msg.block()) {
                            Ok(certificate) => {
                                let rb_msg = RbMsg::new(msg, certificate);
                                self.to_network
                                    .send_ephemera_event(EphemeraEvent::ProtocolMessage(
                                        rb_msg.into(),
                                    ))
                                    .await?;
                            }
                            Err(err) => {
                                return Err(anyhow!("Error signing block: {:?}", err).into());
                            }
                        }
                    }
                    BroadcastResponse::Deliver(hash) => {
                        debug!("Block broadcast complete: {hash:?}",);
                        let block = self.block_manager.get_block_by_hash(&hash);
                        match block {
                            Some(block) => {
                                if block.header.creator == self.node_info.peer_id {
                                    debug!("It's local block: {hash:?}",);

                                    //DB
                                    let certificates = self
                                        .block_manager
                                        .get_block_certificates(&block.header.hash)
                                        .ok_or(anyhow!(
                                            "Error: Block signatures not found in block manager"
                                        ))?;

                                    self.storage
                                        .lock()
                                        .await
                                        .store_block(&block, certificates)?;

                                    //BlockManager
                                    self.block_manager.on_block_committed(&block).map_err(|e| {
                                        anyhow!(
                                            "Error: BlockManager failed to process block: {:?}",
                                            e
                                        )
                                    })?;

                                    //Application(ABCI)
                                    self.application
                                        .deliver_block(Into::into(block.clone()))
                                        .map_err(|e| {
                                            anyhow!(
                                                "Error: Deliver block to Application failed: {e:?}",
                                            )
                                        })?;

                                    //WS
                                    self.ws_message_broadcast.send_block(&block)?;
                                }
                            }
                            None => {
                                return Err(
                                    anyhow!("Error: Block not found in block manager").into()
                                );
                            }
                        }
                    }
                    BroadcastResponse::Drop(hash) => {
                        trace!("Ignoring broadcast message {:?}[block {:?}]", msg_id, hash);
                        return Ok(());
                    }
                }
            }
            Err(err) => {
                error!("Error handling broadcast message: {:?}", err);
            }
        }
        Ok(())
    }

    pub(crate) fn broadcast_group(&mut self) -> &HashSet<PeerId> {
        self.broadcast_group.current()
    }
}
