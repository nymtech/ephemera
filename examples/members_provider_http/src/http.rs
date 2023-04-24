use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

use ephemera::membership::JsonPeerInfo;

use crate::{EPHEMERA_IP, PEERS_API_PORT};

pub(crate) async fn run_peers_http_server(peers_ch: Sender<oneshot::Sender<Vec<JsonPeerInfo>>>) {
    let mut app = tide::with_state(peers_ch.clone());

    app.at("/peers").get(
        |req: tide::Request<Sender<oneshot::Sender<Vec<JsonPeerInfo>>>>| async move {
            let tx = req.state();
            let (reply_tx, reply_rcv) = oneshot::channel();
            tx.send(reply_tx).await.unwrap();
            match reply_rcv.await {
                Ok(reply) => {
                    let str = serde_json::to_string(&reply).unwrap();
                    Ok(str)
                }
                Err(err) => {
                    println!("Error: {:?}", err);
                    Ok("[]".to_string())
                }
            }
        },
    );

    app.listen(format!("{}:{}", EPHEMERA_IP, PEERS_API_PORT))
        .await
        .unwrap();
}
