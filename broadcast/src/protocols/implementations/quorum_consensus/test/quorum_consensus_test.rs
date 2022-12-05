//Test consensus state machine internally.
// It is not concerned about networking nor time etc.

//Tests aren't very comprehensive yet because the exact formal requirements aren't specified for the time being.
#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use std::path::Path;
    use std::time;

    use anyhow::Result;
    use prost_types::Timestamp;
    use serde::Deserialize;

    use uuid::Uuid;

    use crate::network::peer_discovery::{StaticPeerDiscovery, Topology};
    use crate::protocols::implementations::quorum_consensus::quorum::BasicQuorum;
    use crate::protocols::implementations::quorum_consensus::quorum_consensus::{
        QuorumConsensusBroadcastProtocol, QuorumProtocolError,
    };
    use crate::protocols::implementations::quorum_consensus::quorum_consensus_callback::QuorumConsensusCallBack;
    use crate::protocols::protocol::{Kind, Protocol, ProtocolRequest, ProtocolResponse};
    use crate::request::rb_msg::ReliableBroadcast::{Commit, PrePrepare, Prepare};
    use crate::request::{CommitMsg, PrePrepareMsg, PrepareMsg, RbMsg};
    use crate::settings::Settings;

    type ProtocolResult = Result<ProtocolResponse<RbMsg>, QuorumProtocolError>;

    #[derive(Debug, Default)]
    struct DummyCallback {}

    impl QuorumConsensusCallBack<RbMsg, RbMsg, Vec<u8>> for DummyCallback {}

    struct PeerTestInstance {
        protocol: QuorumConsensusBroadcastProtocol<RbMsg, RbMsg, Vec<u8>>,
    }

    impl PeerTestInstance {
        fn new(quorum: BasicQuorum, peer_discovery: StaticPeerDiscovery, settings: Settings) -> Self {
            let callback = DummyCallback::default();
            let protocol = QuorumConsensusBroadcastProtocol::new(
                Box::new(peer_discovery),
                Box::new(quorum),
                Box::new(callback),
                settings,
            );
            PeerTestInstance { protocol }
        }
    }

    struct TestSuite {
        quorum: BasicQuorum,
        peers: HashMap<String, PeerTestInstance>,
    }

    impl TestSuite {
        fn new() -> Self {
            let quorum = BasicQuorum::new(3);
            let peer_settings = init_settings();

            let mut peers = HashMap::new();
            for (id, settings) in peer_settings.into_iter() {
                peers.insert(
                    id.clone(),
                    TestSuite::new_peer(id.clone(), settings, quorum.clone()),
                );
            }
            Self { quorum, peers }
        }

        fn new_peer(_peer_id: String, settings: Settings, quorum: BasicQuorum) -> PeerTestInstance {
            let topology = Topology::new(&settings);
            let peer_discovery = StaticPeerDiscovery::new(topology);
            PeerTestInstance::new(quorum.clone(), peer_discovery, settings.clone())
        }

        fn get_peer(&mut self, peer_id: &str) -> &mut PeerTestInstance {
            self.peers.get_mut(peer_id).unwrap()
        }

        async fn broadcast_message_from_peer(
            &mut self,
            from: &str,
            msg: ProtocolRequest<RbMsg>,
        ) -> Vec<ProtocolResponse<RbMsg>> {
            let mut responses = vec![];
            for (id, peer) in self.peers.iter_mut() {
                if id != from {
                    let response = peer.protocol.handle(msg.clone()).await.unwrap();
                    responses.push(response);
                }
            }
            responses
        }

        pub(crate) fn pre_prepare_msg(
            &mut self,
            id: String,
            from: String,
            payload: Vec<u8>,
        ) -> ProtocolRequest<RbMsg> {
            let timestamp = Some(Timestamp::from(time::SystemTime::now()));
            let rbm = RbMsg {
                id,
                node_id: from.clone(),
                timestamp,
                reliable_broadcast: Some(PrePrepare(PrePrepareMsg { payload })),
            };
            ProtocolRequest::new(from, rbm)
        }

        pub(crate) fn prepare_msg(
            &mut self,
            msg_id: String,
            from: String,
            payload: Vec<u8>,
        ) -> ProtocolRequest<RbMsg> {
            let timestamp = Some(Timestamp::from(time::SystemTime::now()));
            let rbm = RbMsg {
                id: msg_id,
                node_id: from.clone(),
                timestamp,
                reliable_broadcast: Some(Prepare(PrepareMsg { payload })),
            };
            ProtocolRequest::new(from, rbm)
        }

        pub(crate) fn commit_msg(&mut self, msg_id: String, from: String) -> ProtocolRequest<RbMsg> {
            let timestamp = Some(Timestamp::from(time::SystemTime::now()));
            let rbm = RbMsg {
                id: msg_id,
                node_id: from.clone(),
                timestamp,
                reliable_broadcast: Some(Commit(CommitMsg {})),
            };
            ProtocolRequest::new(from, rbm)
        }

        async fn send_pre_prepare(&mut self, to: &str, payload: Vec<u8>) -> ProtocolResult {
            let msg = self.pre_prepare_msg(Uuid::new_v4().to_string(), "client".to_string(), payload);
            let peer = self.peers.get_mut(to).unwrap();
            peer.protocol.handle(msg).await
        }

        async fn send_prepare(
            &mut self,
            msg_id: String,
            to: &str,
            from: String,
            payload: Vec<u8>,
        ) -> ProtocolResult {
            let msg = self.prepare_msg(msg_id, from, payload);
            let peer = self.peers.get_mut(to).unwrap();
            peer.protocol.handle(msg).await
        }

        async fn send_commit(&mut self, msg_id: String, to: &str, from: String) -> ProtocolResult {
            let msg = self.commit_msg(msg_id, from);
            let peer = self.peers.get_mut(to).unwrap();
            peer.protocol.handle(msg).await
        }
    }

    async fn init_topology(settings: Settings) -> Topology {
        Topology::new(&settings)
    }

    #[derive(Deserialize, Debug, Clone)]
    struct PeerSettings {
        peers: HashMap<String, Settings>,
    }

    fn init_settings() -> HashMap<String, Settings> {
        let file = config::File::from(Path::new(
            "src/protocols/implementations/quorum_consensus/test/config.toml",
        ));
        let config = config::Config::builder()
            .add_source(file)
            .build()
            .expect("Failed to load configuration");

        config
            .try_deserialize()
            .expect("Failed to deserialize configuration")
    }

    #[tokio::test]
    async fn test_pre_prepare_response() {
        let mut suite = TestSuite::new();
        let payload = "Hello".as_bytes().to_vec();

        let response = suite.send_pre_prepare("peer1", payload.clone()).await.unwrap();

        assert_eq!(response.kind, Kind::BROADCAST);
        assert_eq!(response.peers, vec!["peer2", "peer3"]);
        assert_eq!(response.protocol_reply.node_id, "peer1");
        let rbm = response.protocol_reply.reliable_broadcast.unwrap();
        assert_eq!(rbm, Prepare(PrepareMsg { payload }));
    }

    #[tokio::test]
    async fn test_prepare_response() {
        let mut suite = TestSuite::new();
        let payload = "Hello".as_bytes().to_vec();
        let id = Uuid::new_v4().to_string();

        let response = suite
            .send_prepare(id, "peer1", "peer".to_string(), payload.clone())
            .await
            .unwrap();

        assert_eq!(response.kind, Kind::BROADCAST);
        assert_eq!(response.peers, vec!["peer2", "peer3"]);
        assert_eq!(response.protocol_reply.node_id, "peer1");
        let rbm = response.protocol_reply.reliable_broadcast.unwrap();
        assert_eq!(rbm, Prepare(PrepareMsg { payload }));
    }

    #[tokio::test]
    #[should_panic(expected = "UnknownBroadcast")]
    async fn test_commit_response_without_prepare() {
        let mut suite = TestSuite::new();
        let _payload = "Hello".as_bytes().to_vec();

        suite
            .send_commit("id".to_string(), "peer1", "unknown".to_string())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_commit_response() {
        let mut suite = TestSuite::new();
        let payload = "Hello".as_bytes().to_vec();
        let id = Uuid::new_v4().to_string();

        let prepare_resp = suite
            .send_prepare(id, "peer1", "peer".to_string(), payload.clone())
            .await
            .unwrap();
        let msg_id = prepare_resp.protocol_reply.id;
        let response = suite
            .send_commit(msg_id, "peer1", "unknown".to_string())
            .await
            .unwrap();

        assert_eq!(response.kind, Kind::BROADCAST);
        assert_eq!(response.peers, vec!["peer2", "peer3"]);
        assert_eq!(response.protocol_reply.node_id, "peer1");
        let rbm = response.protocol_reply.reliable_broadcast.unwrap();
        assert_eq!(rbm, Commit(CommitMsg {}));
    }

    #[tokio::test]
    async fn test_prepared_after_quorum_threshold() {
        let mut suite = TestSuite::new();
        let payload = "Hello".as_bytes().to_vec();
        let id = Uuid::new_v4().to_string();

        let _ = suite
            .send_prepare(id.clone(), "peer1", "peer2".to_string(), payload.clone())
            .await
            .unwrap();

        let peer1 = suite.get_peer("peer1");
        let ctx = peer1.protocol.contexts.get(&id).unwrap();
        assert_eq!(ctx.prepared, false);
        let _ = suite
            .send_prepare(id.clone(), "peer1", "peer3".to_string(), payload.clone())
            .await
            .unwrap();

        let peer1 = suite.get_peer("peer1");
        let ctx = peer1.protocol.contexts.get(&id).unwrap();
        assert_eq!(ctx.prepared, true);
    }

    #[tokio::test]
    async fn test_committed_after_quorum_threshold() {
        let mut suite = TestSuite::new();
        let payload = "Hello".as_bytes().to_vec();
        let id = Uuid::new_v4().to_string();

        let _ = suite
            .send_prepare(id.clone(), "peer1", "peer2".to_string(), payload.clone())
            .await
            .unwrap();

        let peer1 = suite.get_peer("peer1");
        let ctx = peer1.protocol.contexts.get(&id).unwrap();
        assert_eq!(ctx.prepared, false);
        let _ = suite
            .send_prepare(id.clone(), "peer1", "peer3".to_string(), payload.clone())
            .await
            .unwrap();

        let peer1 = suite.get_peer("peer1");
        let ctx = peer1.protocol.contexts.get(&id).unwrap();
        assert_eq!(ctx.prepared, true);

        let _ = suite
            .send_commit(id.clone(), "peer1", "peer2".to_string())
            .await
            .unwrap();

        let peer1 = suite.get_peer("peer1");
        let ctx = peer1.protocol.contexts.get(&id).unwrap();
        assert_eq!(ctx.committed, false);
        let _ = suite
            .send_commit(id.clone(), "peer1", "peer3".to_string())
            .await
            .unwrap();

        let peer1 = suite.get_peer("peer1");
        let ctx = peer1.protocol.contexts.get(&id).unwrap();
        assert_eq!(ctx.committed, true);
    }
}
