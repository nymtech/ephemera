use std::sync::Arc;

use futures_util::StreamExt;
use tokio::sync::Mutex;

use crate::api::application::Application;
use crate::api::ApiListener;
use crate::block::manager::BlockManager;
use crate::block::types::block::Block;
use crate::broadcast::bracha::broadcaster::{Broadcaster, ProtocolResponse};
use crate::broadcast::RbMsg;
use crate::core::api_cmd::ApiCmdProcessor;
use crate::core::builder::{EphemeraHandle, NodeInfo};
use crate::core::shutdown::ShutdownManager;
use crate::network::libp2p::ephemera_sender::{EphemeraEvent, EphemeraToNetworkSender};
use crate::network::libp2p::network_sender::{NetCommunicationReceiver, NetworkEvent};
use crate::storage::EphemeraDatabase;
use crate::utilities::crypto::Certificate;
use crate::websocket::ws_manager::WsMessageBroadcaster;

pub struct Ephemera<A: Application> {
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

    //It is inside option so that it can taken out
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
                    log::trace!("New block from block manager: {}", new_block);
                    match self.application.check_block(&new_block.clone().into()) {
                        Ok(accept) => {
                            if accept {
                                match self.process_new_local_block(new_block, certificate).await {
                                    Ok(_) => {
                                        log::debug!("New block processed successfully");
                                        self.block_manager.accept_last_proposed_block();
                                    }
                                    Err(err) => {
                                        log::error!("Error processing new local block: {:?}", err);
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            log::error!("Error processing new local block: {:?}", err);
                        }
                    }
                }

                // PROCESSING NETWORK EVENTS
                Some(net_event) = self.from_network.net_event_rcv.recv() => {
                    log::trace!("New network event: {:?}", net_event);
                    self.process_network_event(net_event).await;
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

    async fn process_network_event(&mut self, net_event: NetworkEvent) {
        match net_event {
            NetworkEvent::EphemeraMessage(em) => {
                // 1. Ask application to decide if we should accept this message.
                let api_msg = (*em.clone()).into();
                log::debug!("New ephemera message from network: {:?}", api_msg);
                match self.application.check_tx(api_msg) {
                    Ok(true) => {
                        log::trace!("Application accepted message: {:?}", em);
                        // 2. send to BlockManager
                        if let Err(err) = self.block_manager.on_new_message(*em).await {
                            log::error!("Error sending signed message to block manager: {:?}", err);
                        }
                    }
                    Ok(false) => {
                        log::trace!("Application rejected message: {:?}", em);
                    }
                    Err(err) => {
                        log::error!("Application check_tx failed: {:?}", err);
                    }
                }
            }
            NetworkEvent::ProtocolMessage(pm) => {
                log::trace!("New protocol message from network: {:?}", pm);
                if let Err(err) = self.process_block_from_network(*pm).await {
                    log::error!("Error processing block from network: {:?}", err);
                }
            }
            NetworkEvent::PeersUpdated(peers) => {
                if let Err(err) = self.broadcaster.topology_updated(peers).await {
                    log::error!("Error updating broadcaster topology: {:?}", err);
                }
            }
            NetworkEvent::QueryDhtResponse { key, value } => {
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
    }

    async fn process_new_local_block(
        &mut self,
        new_block: Block,
        certificate: Certificate,
    ) -> anyhow::Result<()> {
        //Block manager generated new block that nobody hasn't seen yet.
        //We start reliable broadcaster protocol to broadcaster it to other nodes.

        //1. send to local RB
        match self.broadcaster.new_broadcast(new_block).await {
            Ok(ProtocolResponse {
                status: _,
                command,
                protocol_reply: Some(rp),
            }) => {
                use crate::broadcast::Command;
                if command == Command::Broadcast {
                    log::trace!("Broadcasting new block: {:?}", rp);
                    let rb_msg = RbMsg::new(rp, certificate);
                    //2. send to network
                    self.to_network
                        .send_ephemera_event(EphemeraEvent::ProtocolMessage(rb_msg.into()))
                        .await?;
                }
            }
            Ok(_) => {
                log::error!("Unexpected broadcast")
            }
            Err(err) => {
                log::error!("Error sending new block to broadcaster: {err:?}",);
            }
        }
        Ok(())
    }

    async fn process_block_from_network(&mut self, msg: RbMsg) -> anyhow::Result<()> {
        let msg_id = msg.id.clone();
        let block = msg.get_block();
        let hash = block.header.hash;
        log::trace!(
            "Processing message {:?}[block {:?}] from network",
            msg_id,
            hash
        );
        //Item: PREPARE DOESN'T NEED TO SEND US FULL BLOCK IF IT'S OUR OWN BLOCK.
        //To improve performance, the other nodes technically don't need to send us block back if we created it.
        //But for now it's fine.

        //Item: WE COULD POTENTIALLY VERIFY THAT PUBLIC KEY OF SENDER IS IN ALLOW LIST.
        //Verify that block is signed correctly.
        //We don't verify if sender's public keys in allow list(assume such a thing for security could exist)
        let certificate = msg.certificate.clone();
        if let Err(err) = self.block_manager.on_block(block, &certificate) {
            log::error!("Error processing block: {:?}", err);
        }

        //Item: WE COULD POTENTIALLY CLEAR OUR MEMORY POOL OF TRANSACTIONS THAT ARE ALREADY INCLUDED IN A "FOREIGN" BLOCK.
        //Which also means abandoning the the creation our own block which includes some of those transactions.

        //Send to local RB protocol
        use crate::broadcast::{Command, Status};
        match self.broadcaster.handle(msg.into()).await {
            Ok(ProtocolResponse {
                status: _,
                command: Command::Broadcast,
                protocol_reply: Some(rp),
            }) => {
                //Send protocol response to network
                log::trace!("Broadcasting block to network: {:?}", rp);
                let certificate = self.block_manager.sign_block(rp.get_block())?;
                let rb_msg = RbMsg::new(rp, certificate);
                self.to_network
                    .send_ephemera_event(EphemeraEvent::ProtocolMessage(rb_msg.into()))
                    .await?;
            }
            Ok(ProtocolResponse {
                status: Status::Completed,
                command: _,
                protocol_reply: None,
            }) => {
                //If committed, store in DB, send to WS
                let block = self.block_manager.get_block_by_hash(&hash);

                match block {
                    Some(block) => {
                        if block.header.creator == self.node_info.peer_id {
                            //DB
                            let signatures = self
                                .block_manager
                                .get_block_signatures(&block.header.hash)
                                .expect("Error getting block signatures");

                            self.storage
                                .lock()
                                .await
                                .store_block(&block, signatures)
                                .expect("Error storing block");

                            //Application(ABCI)
                            if let Err(err) =
                                self.application.deliver_block(Into::into(block.clone()))
                            {
                                log::error!("Error delivering block to ABCI: {:?}", err);
                            }

                            //BlockManager
                            if let Err(err) =
                                self.block_manager.on_block_committed(&block.header.hash)
                            {
                                log::error!(
                                    "Error notifying BlockManager about block commit: {:?}",
                                    err
                                );
                            }

                            //WS
                            if let Err(err) = self.ws_message_broadcast.send_block(&block) {
                                log::error!("Error sending block to WS: {:?}", err);
                            }
                        }
                    }
                    None => {
                        log::error!("Block not found in BlockManager: {:?}", hash)
                    }
                }
            }
            Ok(_) => {
                log::trace!("Dropping ack the broadcast message")
            }
            Err(e) => log::error!("Error: {}", e),
        }
        Ok(())
    }
}
