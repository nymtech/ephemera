use std::sync::Arc;

use anyhow::anyhow;
use futures_util::StreamExt;
use thiserror::Error;
use tokio::sync::Mutex;

use crate::{
    api::{ApiListener, application::Application},
    block::{manager::BlockManager, types::block::Block},
    broadcast::{
        bracha::broadcaster::{Broadcaster, ProtocolResponse},
        Command, RbMsg, Status,
    },
    core::{
        api_cmd::ApiCmdProcessor,
        builder::{EphemeraHandle, NodeInfo},
        shutdown::ShutdownManager,
    },
    network::{
        libp2p::{
            ephemera_sender::{EphemeraEvent, EphemeraToNetworkSender},
            network_sender::{NetCommunicationReceiver, NetworkEvent},
        },
        topology::BroadcastTopology,
    },
    storage::EphemeraDatabase,
    utilities::crypto::Certificate,
    websocket::ws_manager::WsMessageBroadcaster,
};
use crate::api::application::CheckBlockResult;

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

    /// A component which keeps track of network topology over time.
    pub(crate) topology: BroadcastTopology,

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
    /// 2. Receive new ephemera messages(transactions) from network.
    /// 3. Receive new protocol messages(blocks) from network.
    /// 4. Receive http api request
    /// 5. Receive rust api request
    /// 6. Publish ephemera messages to network
    /// 7. Publish broadcast messages to network
    ///
    /// It works in a loop like nodejs event loop. At current stage of development I haven't analyzed
    /// how this workflow affects the overall system performance and its impact to each affected component
    /// individually. Because it's not expected(at its current scope) to be an extremely high performance
    /// system, I'm not worried about it at this stage.
    pub async fn run(mut self) {
        log::info!("Starting ephemera...");

        let mut shutdown_manager = self
            .shutdown_manager
            .take()
            .expect("Shutdown manager not set");

        loop {
            tokio::select! {
                // GENERATING NEW BLOCKS
                Some((new_block, certificate)) = self.block_manager.next() => {
                    if let Err(err) = self.process_new_local_block(new_block, certificate).await{
                        log::error!("Error processing new block: {:?}", err);
                    }
                }

                // PROCESSING NETWORK EVENTS
                Some(net_event) = self.from_network.net_event_rcv.recv() => {
                    if let Err(err) = self.process_network_event(net_event).await{
                        log::error!("Error processing network event: {:?}", err);
                    }
                }

                //PROCESSING EXTERNAL API REQUESTS
                api = self.api_listener.messages_rcv.recv() => {
                    match api {
                        Some(api_msg) => {
                            if let Err(err) = ApiCmdProcessor::process_api_requests(&mut self, api_msg).await{
                                log::error!("Error processing api request: {:?}", err);
                            }
                        }
                        None => {
                            log::error!("Error: Api listener channel closed");
                            //TODO: handle shutdown
                        }
                    }
                }

                //PROCESSING SHUTDOWN REQUEST
                _ = shutdown_manager.external_shutdown.recv() => {
                    log::info!("Shutting down ephemera");
                    shutdown_manager.stop().await;
                    break;
                }
            }
        }
    }

    async fn process_network_event(&mut self, net_event: NetworkEvent) -> Result<()> {
        log::trace!("New network event: {:?}", net_event);

        match net_event {
            NetworkEvent::EphemeraMessage(em) => {
                // 1. Ask application to decide if we should accept this message.
                let api_msg = (*em.clone()).into();
                log::debug!("New ephemera message from network: {:?}", api_msg);
                match self.application.check_tx(api_msg) {
                    Ok(true) => {
                        log::debug!("Application accepted message: {:?}", em);
                        // 2. send to BlockManager
                        if let Err(err) = self.block_manager.on_new_message(*em) {
                            log::error!("Error sending signed message to block manager: {:?}", err);
                        }
                    }
                    Ok(false) => {
                        log::debug!("Application rejected message: {:?}", em);
                    }
                    Err(err) => {
                        log::error!("Application check_tx failed: {:?}", err);
                    }
                }
            }
            NetworkEvent::ProtocolMessage(pm) => {
                log::debug!("New protocol message from network: {:?}", pm);
                if let Err(err) = self.process_block_from_network(*pm).await {
                    log::error!("Error processing block from network: {:?}", err);
                }
            }
            NetworkEvent::PeersUpdated(peers) => {
                log::debug!("New peers: {:?}", peers);
                self.broadcaster.topology_updated(peers.len());
                self.topology.add_snapshot(peers);
            }
            NetworkEvent::QueryDhtResponse { key, value } => {
                log::debug!("New dht query response: {:?}", key);
                match self.api_cmd_processor.dht_query_cache.pop(&key) {
                    Some(reply) => {
                        let response = Ok(Some((key, value)));
                        if let Err(err) = reply.send(response) {
                            log::error!("Error sending dht query response: {:?}", err);
                        }
                    }
                    None => {
                        log::error!(
                            "Error: No dht query cache found for key: {:?}",
                            String::from_utf8(key)
                        );
                    }
                }
            }
        }
        Ok(())
    }

    async fn process_new_local_block(
        &mut self,
        new_block: Block,
        certificate: Certificate,
    ) -> Result<()> {
        log::debug!("New block from block manager: {}", new_block);

        let hash = new_block.header.hash;
        let block_creator = &self.node_info.peer_id;
        let sender = &self.node_info.peer_id;

        // Check if block matches topology
        if !self.topology.check_membership(hash, block_creator, sender) {
            log::debug!("Membership check rejected block: {:?}", new_block);
            return Ok(());
        }

        //Ephemera ABCI
        match self.application.check_block(&new_block.clone().into()) {
            Ok(response) => match response {
                CheckBlockResult::Accept => {
                    log::debug!("Application accepted block: {:?}", new_block);
                }
                CheckBlockResult::Reject => {
                    log::debug!("Application rejected block: {hash:?}",);
                    return Ok(());
                }
                CheckBlockResult::RejectAndRemoveMessages(messages_to_remove) => {
                    log::debug!("Application rejected block: {:?}", messages_to_remove);
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
            Ok(ProtocolResponse {
                   status: _,
                   command,
                   protocol_reply: Some(rp),
               }) => {
                if command == Command::Broadcast {
                    log::trace!("Broadcasting new block: {:?}", rp);

                    let rb_msg = RbMsg::new(rp, certificate);
                    self.to_network
                        .send_ephemera_event(EphemeraEvent::ProtocolMessage(rb_msg.into()))
                        .await?;
                }
            }
            Ok(_) => {
                return Err(anyhow!("Error sending new block to broadcaster").into());
            }
            Err(err) => {
                return Err(anyhow!("Error sending new block to broadcaster: {err:?}").into());
            }
        }
        Ok(())
    }

    async fn process_block_from_network(&mut self, msg: RbMsg) -> Result<()> {
        log::debug!("New broadcast message from network: {:?}", msg);

        let msg_id = msg.id.clone();
        let block = msg.get_block();
        let block_creator = &block.header.creator;
        let sender = &msg.original_sender;
        let hash = block.header.hash;
        let certificate = msg.certificate.clone();

        if !self.topology.check_membership(hash, block_creator, sender) {
            return Err(anyhow!("Block doesn't match topology").into());
        }

        if let Err(err) = self.block_manager.on_block(sender, block, &certificate) {
            return Err(anyhow!("Error sending block to block manager: {:?}", err).into());
        }

        //Send to local RB protocol
        match self.broadcaster.handle(msg.into()).await {
            Ok(ProtocolResponse {
                   status: _,
                   command: Command::Broadcast,
                   protocol_reply: Some(rp),
               }) => {
                log::trace!("Broadcasting block to network: {:?}", rp);

                match self.block_manager.sign_block(rp.block_ref()) {
                    Ok(certificate) => {
                        let rb_msg = RbMsg::new(rp, certificate);
                        self.to_network
                            .send_ephemera_event(EphemeraEvent::ProtocolMessage(rb_msg.into()))
                            .await?;
                    }
                    Err(err) => {
                        return Err(anyhow!("Error signing block: {:?}", err).into());
                    }
                }
            }
            Ok(ProtocolResponse {
                   status: Status::Completed,
                   command: _,
                   protocol_reply: None,
               }) => {
                log::debug!("Block broadcast complete: {hash:?}",);
                let block = self.block_manager.get_block_by_hash(&hash);
                match block {
                    Some(block) => {
                        if block.header.creator == self.node_info.peer_id {
                            log::debug!("It's local block: {hash:?}",);

                            //DB
                            let signatures = self
                                .block_manager
                                .get_block_certificates(&block.header.hash)
                                .ok_or(anyhow!(
                                    "Error: Block signatures not found in block manager"
                                ))?;

                            self.storage.lock().await.store_block(&block, signatures)?;

                            //BlockManager
                            self.block_manager.on_block_committed(&block).map_err(|e| {
                                anyhow!("Error: BlockManager failed to process block: {:?}", e)
                            })?;

                            //Application(ABCI)
                            self.application
                                .deliver_block(Into::into(block.clone()))
                                .map_err(|e| {
                                    anyhow!("Error: Deliver block to Application failed: {e:?}",)
                                })?;

                            //WS
                            self.ws_message_broadcast.send_block(&block)?;
                        }
                    }
                    None => {
                        return Err(anyhow!("Error: Block not found in block manager").into());
                    }
                }
            }
            Ok(_) => {
                log::trace!("Ignoring broadcast message {:?}[block {:?}]", msg_id, hash);
                return Ok(());
            }
            Err(e) => log::error!("Error: {}", e),
        };
        Ok(())
    }
}
