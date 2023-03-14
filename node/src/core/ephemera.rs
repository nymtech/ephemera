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
use crate::network::libp2p::messages_channel::{NetCommunicationReceiver, NetCommunicationSender};
use crate::storage::rocksdb::RocksDbStorage;
use crate::storage::EphemeraDatabase;
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

    pub(crate) from_network: NetCommunicationReceiver,

    pub(crate) to_network: NetCommunicationSender,

    /// A component which has mutable access to database.
    pub(crate) storage: Arc<Mutex<RocksDbStorage>>,

    /// A component which broadcasts messages to websocket clients.
    pub(crate) ws_message_broadcast: WsMessageBroadcaster,

    /// A component which listens API requests.
    pub(crate) api_listener: ApiListener,

    /// An implementation of Application trait. Provides callbacks to broadcast.
    pub(crate) application: Arc<A>,

    pub(crate) ephemera_handle: EphemeraHandle,

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
                Some(new_block) = self.block_manager.next() => {
                    log::trace!("New block from block manager: {}", new_block);
                    if let Err(err) = self.process_new_local_block(new_block).await{
                        log::error!("Error processing new local block: {:?}", err);
                    }
                }

                //PROCESSING EPHEMERA MESSAGES(TRANSACTIONS) FROM NETWORK
                Some(ephemera_msg) = self.from_network.ephemera_message_receiver.recv() => {
                    // 1. Ask application to decide if we should accept this message.
                    let msd_id = ephemera_msg.id.clone();
                    let api_msg = ephemera_msg.clone().into();
                    log::debug!("New ephemera message from network: {}", msd_id);
                    match self.application.check_tx(api_msg){
                        Ok(true) => {
                            log::debug!("Application accepted message: {}", msd_id);
                            // 2. send to BlockManager
                            if let Err(err) = self.block_manager.new_message(ephemera_msg).await{
                                log::error!("Error sending signed message to block manager: {:?}", err);
                            }
                        }
                        Ok(false) => {
                            log::debug!("Application rejected message: {:?}", msd_id);
                        }
                        Err(err) => {
                            log::error!("Application rejected message: {:?}", err);
                            continue;
                        }
                    }
                }

                //PROCESSING PROTOCOL MESSAGES(BLOCKS) FROM NETWORK
                Some(network_msg) = self.from_network.protocol_msg_receiver.recv() => {
                    log::trace!("New protocol message from network: {:?}", network_msg);
                    if let Err(err) = self.process_block_from_network(network_msg).await{
                        log::error!("Error processing block from network: {:?}", err);
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
                        }
                    }
                }
                _ = shutdown_manager.external_shutdown.recv() => {
                    log::info!("Shutting down ephemera");
                    shutdown_manager.stop().await;
                    break;
                }
            }
        }
    }

    async fn process_new_local_block(&mut self, new_block: Block) -> anyhow::Result<()> {
        match self.application.accept_block(&new_block.clone().into()) {
            Ok(accept) => {
                if !accept {
                    log::debug!("Application rejected new block: {}", new_block.header.id);
                    return Ok(());
                }
            }
            Err(err) => anyhow::bail!("Error checking if application accepts new block: {:?}", err),
        }

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
                    //2. send to network
                    self.to_network.send_protocol_message(rp).await?;
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
        let block_id = msg.data_identifier.clone();
        log::trace!(
            "Processing message {}[block {}] from network",
            msg_id,
            block_id
        );
        //Item: PREPARE DOESN'T NEED TO SEND US FULL BLOCK IF IT'S OUR OWN BLOCK.
        //To improve performance, the other nodes technically don't need to send us block back if we created it.
        //But for now it's fine.

        //Item: WE COULD POTENTIALLY VERIFY THAT PUBLIC KEY OF SENDER IS IN ALLOW LIST.
        //Verify that block is signed correctly.
        //We don't verify if sender's public keys in allow list(assume such a thing for security could exist)
        let block = msg.get_block();
        if let Err(err) = self.block_manager.on_block(block) {
            log::error!("Error processing block: {:?}", err);
        }

        //Item: WE COULD POTENTIALLY CLEAR OUR MEMORY POOL OF TRANSACTIONS THAT ARE ALREADY INCLUDED IN A "FOREIGN" BLOCK.
        //Which also means abandoning the the creation our own block which includes some of those transactions.

        //Send to local RB protocol
        use crate::broadcast::{Command, Status};
        match self.broadcaster.handle(msg).await {
            Ok(ProtocolResponse {
                status: _,
                command: Command::Broadcast,
                protocol_reply: Some(rp),
            }) => {
                //Send protocol response to network
                log::trace!("Broadcasting block to network: {:?}", rp);
                self.to_network.send_protocol_message(rp.clone()).await?
            }
            Ok(ProtocolResponse {
                status: Status::Committed,
                command: _,
                protocol_reply: None,
            }) => {
                //If committed, store in DB, send to WS
                let block = self.block_manager.get_block_by_id(&block_id);

                match block {
                    Some(block) => {
                        if block.header.creator == self.node_info.peer_id {
                            //DB
                            let signatures = self
                                .broadcaster
                                .get_block_signatures(&block.header.id)
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
                                self.block_manager.on_block_committed(&block.header.id)
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
                        log::error!("Block not found in BlockManager: {:?}", block_id)
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
