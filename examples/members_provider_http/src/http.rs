use crate::{ReducingPeerProvider, EPHEMERA_IP, PEERS_API_PORT};
use std::sync::Arc;

pub(crate) async fn run_peers_http_server(provider: ReducingPeerProvider) {
    let mut app = tide::with_state(Arc::new(provider));

    app.at("/peers")
        .get(|req: tide::Request<Arc<ReducingPeerProvider>>| async move {
            let provider = req.state();
            let str = serde_json::to_string(&provider.peers()).unwrap();
            Ok(str)
        });

    app.listen(format!("{}:{}", EPHEMERA_IP, PEERS_API_PORT))
        .await
        .unwrap();
}
