use std::sync::Arc;

use futures_util::StreamExt;
use tokio::sync::Mutex;

use crate::api::{ApiCmd, ApiListener, EphemeraExternalApi};
use crate::block::callback::DummyBlockProducerCallback;
use crate::block::manager::BlockManager;
use crate::broadcast::broadcaster::Broadcaster;
use crate::config::configuration::Configuration;
use crate::database::{CompoundDatabase, EphemeraDatabase};
use crate::http;
use crate::network::libp2p::listener::{MessagesReceiver, NetworkBroadcastReceiver};
use crate::network::libp2p::sender::EphemeraMessagesNotifier;
use crate::network::libp2p::swarm::SwarmNetwork;
use crate::utilities::crypto::read_keypair;
use crate::websocket::ws_manager::{WsManager, WsMessageBroadcaster};

pub(crate) type EphemeraDatabaseType = CompoundDatabase;

pub struct Ephemera {
    /// Block manager responsibility includes:
    /// - Block production and signing
    /// - Block verification for externally received blocks
    /// - Message verification sent by clients and gossiped other nodes
    pub(crate) block_manager: BlockManager,

    /// Broadcaster is making sure that blocks are deterministically agreed by all nodes.
    pub(crate) broadcaster: Broadcaster,

    /// A component which sends messages to other nodes.
    /// This includes client messages and blocks.
    pub(crate) ephemera_message_notifier: EphemeraMessagesNotifier,

    /// A component which receives messages from other nodes.
    pub(crate) net_messages_notify_receiver: MessagesReceiver,

    /// A component which receives blocks from other nodes.
    pub(crate) protocol_broadcast_receiver: NetworkBroadcastReceiver,

    /// A component which has mutable access to database.
    pub(crate) storage: Arc<Mutex<CompoundDatabase>>,

    /// A component which listens API requests.
    pub(crate) api_listener: ApiListener,

    /// A component which broadcasts messages to websocket clients.
    pub(crate) ws_message_broadcaster: WsMessageBroadcaster,

    pub api: EphemeraExternalApi,
}

impl Ephemera {
    pub async fn start_services(config: Configuration) -> Ephemera {
        let (local_peer_id, keypair) = read_keypair(config.node_config.private_key.clone());
        log::info!("Local node id: {}", local_peer_id);

        log::info!("Starting broadcaster...");
        let broadcaster = Broadcaster::new(config.clone(), local_peer_id, keypair.clone());

        log::info!("Starting storage...");
        let storage = Arc::new(Mutex::new(CompoundDatabase::new(config.db_config.clone())));

        log::info!("Starting block manager...");
        let last_block = storage
            .lock()
            .await
            .get_last_block()
            .expect("Failed to get last block");
        let block_manager = BlockManager::new(
            DummyBlockProducerCallback,
            config.clone(),
            local_peer_id,
            keypair.clone(),
            last_block,
        );

        log::info!("Starting network...");
        let (
            network,
            ephemera_message_notifier,
            net_message_notify_receiver,
            protocol_broadcast_receiver,
        ) = SwarmNetwork::new(config.clone(), keypair.clone());

        tokio::spawn(network.start());

        let (api, api_listener) = EphemeraExternalApi::new(storage.clone());

        log::info!("Starting http server...");
        let server = http::start(config.http_config.clone(), api.clone())
            .expect("Failed to start http server");
        tokio::spawn(server);

        let (ws_manager, ws_message_broadcast) = WsManager::new(config.ws_config.clone()).unwrap();
        tokio::spawn(ws_manager.bind());

        Ephemera {
            block_manager,
            broadcaster,
            ephemera_message_notifier,
            net_messages_notify_receiver: net_message_notify_receiver,
            protocol_broadcast_receiver,
            storage: storage.clone(),
            api_listener,
            ws_message_broadcaster: ws_message_broadcast,
            api,
        }
    }

    pub fn api(&self) -> EphemeraExternalApi {
        self.api.clone()
    }

    /// Main loop of ephemera node.
    /// 1. Block manager generates new blocks.
    /// 2. Receive new signed messages(transactions) from network.
    /// 3. Receive new protocol messages(including new blocks as part of protocol) from network.
    /// 4. Receive new http messages from http server.
    /// 5. Publish messages to network
    pub async fn run(&mut self) {
        log::debug!("Starting ephemera main loop...");
        loop {
            tokio::select! {
                // GENERATING NEW BLOCKS
                Some(new_block) = self.block_manager.next() => {
                    log::debug!("New block from block manager: {}", new_block);

                    //Block manager generated new block that nobody hasn't seen yet.
                    //We start reliable broadcaster protocol to broadcaster it to other nodes.

                    //1. send to local RB
                    match self.broadcaster.new_broadcast(new_block.clone()).await{
                        Ok(rb_result) => {
                            use crate::broadcast::Command;
                            if rb_result.command == Command::Broadcast {
                                //1. send to network
                                self.ephemera_message_notifier.send_protocol_message(rb_result.protocol_reply).await;
                            }
                        }
                        Err(err) => {
                            log::error!("Error sending new block to broadcaster: {:?}", err);
                        }
                    }
                }

                //PROCESSING SIGNED MESSAGES(TRANSACTIONS) FROM NETWORK
                Some(signed_msg) = self.net_messages_notify_receiver.signed_messages_from_net_rcv.recv() => {
                    // 1. send to BlockManager
                    if let Err(err) = self.block_manager.new_message(signed_msg).await{
                        log::error!("Error sending signed message to block manager: {:?}", err);
                    }
                }

                //PROCESSING PROTOCOL MESSAGES(BLOCKS) FROM NETWORK
                Some(network_msg) = self.protocol_broadcast_receiver.rb_msg_from_net_rcv.recv() => {
                    //Item: PREPARE DOESN'T NEED TO SEND US FULL BLOCK IF IT'S OUR OWN BLOCK.
                    //To improve performance, the other nodes technically don't need to send us block back if we created it.
                    //But for now it's fine.

                    //Item: WE COULD POTENTIALLY VERIFY THAT PUBLIC KEY OF SENDER IS IN ALLOW LIST.
                    //Verify that block is signed correctly.
                    //We don't verify if sender's public keys in allow list(assume such a thing for security could exist)
                    if let Some(block) = &network_msg.msg.get_data() {
                        if let Err(err) = self.block_manager.on_block(block,) {
                            log::error!("Error processing block: {:?}", err);
                            continue;
                        }
                    }

                    //Item: WE COULD POTENTIALLY CLEAR OUR MEMORY POOL OF TRANSACTIONS THAT ARE ALREADY INCLUDED IN A "FOREIGN" BLOCK.
                    //Which also means abandoning the the creation our own block which includes some of those transactions.

                    //Send to local RB protocol
                    match self.broadcaster.handle(network_msg.msg).await {
                        Ok(response) => {
                            log::trace!("RB response: {:?}", response);

                            use crate::broadcast::{Status, Command};
                            //Send protocol response to network
                            if response.command == Command::Broadcast {

                                log::trace!("Broadcasting block to network: {:?}", response.protocol_reply);
                                self.ephemera_message_notifier.send_protocol_message(response.protocol_reply.clone()).await;
                            }

                            //If committed, store in DB, send to WS
                            if response.status == Status::Committed {
                                let block = self.block_manager.get_block_by_id(&response.protocol_reply.data_identifier);
                                match block {
                                    Some(block) => {
                                        let signatures = self.broadcaster.get_block_signatures(&block.header.id).expect("Error getting block signatures");
                                        log::info!("Signatures for block {}: {:?}", block.header.id, signatures);
                                        //Store in DB
                                        self.storage.lock().await.store_block(&block, signatures).expect("Error storing block");

                                        //Notify BlockManager to clean up memory pool
                                        if let Err(err) = self.block_manager.on_block_committed(&block.header.id){
                                            log::error!("Error notifying BlockManager about block commit: {:?}", err);
                                        }

                                        //Send to WS
                                        log::trace!("Sending block to WS: {:?}", block);
                                        if let Err(err) = self.ws_message_broadcaster.send_block(&block){
                                            log::error!("Error sending block to WS: {:?}", err);
                                        }
                                    }
                                    None => log::error!("Block not found in BlockManager: {:?}", response.protocol_reply.data_identifier)
                                }
                            }
                        }
                        Err(e) => log::error!("Error: {}", e),
                    }
                }

                //PROCESSING EXTERNAL API REQUESTS
                api = self.api_listener.signed_messages_rcv.recv() => {
                    match api {
                        Some(api_msg) => {
                            log::trace!("Received api message: {:?}", api_msg);
                            match api_msg {
                                ApiCmd::SubmitSignedMessageRequest(sm) => {

                                    //Send to BlockManager for verification and put into memory pool
                                    let signed_message: crate::block::SignedMessage = sm.into();
                                    match self.block_manager.new_message(signed_message.clone()).await {
                                        Ok(_) => {
                                            //Gossip to network for other nodes to receive
                                            self.ephemera_message_notifier.gossip_signed_message(signed_message);
                                        }
                                        Err(e) => log::error!("Error: {}", e),
                                    }
                                }
                            }
                        }
                        None => {
                            log::error!("Error: Api listener channel closed");
                        }
                    }
                }

                //SENDING INTERNAL MESSAGES TO NETWORK
                _ = self.ephemera_message_notifier.flush_timer.tick() => {
                    if let Err(err) = self.ephemera_message_notifier.flush().await{
                        log::error!("Error flushing ephemera message notifier: {:?}", err);
                    }
                }
            }
        }
    }
}
