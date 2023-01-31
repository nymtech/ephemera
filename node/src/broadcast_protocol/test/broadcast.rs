// Test consensus state machine internally.
// It is not concerned about networking nor time etc.

// Tests aren't very comprehensive yet because the exact formal requirements aren't specified for the time being.
#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use std::path::Path;
    use std::time;

    use crate::broadcast_protocol::broadcast::{BroadcastProtocol, ProtocolResult};
    use crate::broadcast_protocol::quorum::BasicQuorum;
    use crate::broadcast_protocol::{BroadcastCallBack, EphemeraSigningRequest, Kind, ProtocolResponse};

    use crate::config::configuration::Configuration;
    use prost_types::Timestamp;
    use uuid::Uuid;

    use crate::request::rb_msg::ReliableBroadcast::{Commit, PrePrepare, Prepare};
    use crate::request::{CommitMsg, PrePrepareMsg, PrepareMsg, RbMsg};

    #[derive(Debug, Default)]
    struct DummyCallback {}

    impl BroadcastCallBack for DummyCallback {}

    struct PeerTestInstance<C: BroadcastCallBack + Send> {
        protocol: BroadcastProtocol<C>,
    }

    impl PeerTestInstance<DummyCallback> {
        fn new(conf: Configuration) -> Self {
            let callback = DummyCallback::default();
            let quorum = BasicQuorum::new(conf.quorum.clone());
            let protocol = BroadcastProtocol::new(Box::new(quorum), callback, conf);
            PeerTestInstance { protocol }
        }
    }

    struct TestSuite {
        peers: HashMap<String, PeerTestInstance<DummyCallback>>,
    }

    impl TestSuite {
        fn new() -> Self {
            let peer_settings = init_settings();
            let mut peers = HashMap::new();
            for (id, settings) in peer_settings.into_iter() {
                peers.insert(id.clone(), TestSuite::new_peer(settings));
            }
            Self { peers }
        }

        fn new_peer(conf: Configuration) -> PeerTestInstance<DummyCallback> {
            PeerTestInstance::new(conf)
        }

        fn get_peer(&mut self, peer_id: &str) -> &mut PeerTestInstance<DummyCallback> {
            self.peers.get_mut(peer_id).unwrap()
        }

        #[allow(dead_code)]
        async fn broadcast_message_from_peer(
            &mut self,
            from: &str,
            msg: EphemeraSigningRequest,
        ) -> Vec<ProtocolResponse> {
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
        ) -> EphemeraSigningRequest {
            let timestamp = Some(Timestamp::from(time::SystemTime::now()));
            let rbm = RbMsg {
                id,
                node_id: from.clone(),
                timestamp,
                custom_message_id: "custom_message_id".to_string(),
                reliable_broadcast: Some(PrePrepare(PrePrepareMsg { payload })),
            };
            EphemeraSigningRequest::new(from, rbm)
        }

        pub(crate) fn prepare_msg(
            &mut self,
            msg_id: String,
            from: String,
            payload: Vec<u8>,
        ) -> EphemeraSigningRequest {
            let timestamp = Some(Timestamp::from(time::SystemTime::now()));
            let rbm = RbMsg {
                id: msg_id,
                custom_message_id: "custom_message_id".to_string(),
                node_id: from.clone(),
                timestamp,
                reliable_broadcast: Some(Prepare(PrepareMsg { payload })),
            };
            EphemeraSigningRequest::new(from, rbm)
        }

        pub(crate) fn commit_msg(&mut self, msg_id: String, from: String) -> EphemeraSigningRequest {
            let timestamp = Some(Timestamp::from(time::SystemTime::now()));
            let rbm = RbMsg {
                id: msg_id,
                custom_message_id: "custom_message_id".to_string(),
                node_id: from.clone(),
                timestamp,
                reliable_broadcast: Some(Commit(CommitMsg {})),
            };
            EphemeraSigningRequest::new(from, rbm)
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

    fn init_settings() -> HashMap<String, Configuration> {
        let file = config::File::from(Path::new("src/broadcast_protocol/test/config.toml"));
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

        assert_eq!(response.kind, Kind::Broadcast);
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

        assert_eq!(response.kind, Kind::Broadcast);
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

        assert_eq!(response.kind, Kind::Broadcast);
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
