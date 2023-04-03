use std::sync::{Arc, Mutex};

use ephemera::ephemera_api::ApiBlock;
use futures::StreamExt;
use reqwest::{IntoUrl, Url};

use crate::Data;

pub(crate) struct WsBlockListener {
    url: Url,
    data: Arc<Mutex<Data>>,
}

impl WsBlockListener {
    pub(crate) fn new<U: IntoUrl>(url: U, data: Arc<Mutex<Data>>) -> WsBlockListener {
        WsBlockListener {
            url: url.into_url().unwrap(),
            data,
        }
    }

    pub(crate) async fn listen(&mut self) {
        let (mut ws_stream, _) = tokio_tungstenite::connect_async(&self.url).await.unwrap();
        loop {
            if let Some(item) = ws_stream.next().await {
                match item {
                    Ok(res) => {
                        println!("Received new block");
                        let block = serde_json::from_str::<ApiBlock>(&res.to_string()).unwrap();
                        self.data.lock().unwrap().received_blocks.push(block);
                    }
                    Err(err) => {
                        println!("Error receiving message: {err:?}",);
                    }
                }
            }
        }
    }
}
