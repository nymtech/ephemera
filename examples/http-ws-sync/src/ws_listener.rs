use std::sync::{Arc, Mutex};
use std::thread;

use futures::TryStreamExt;
use reqwest::{IntoUrl, Url};

use ephemera::ephemera_api::ApiBlock;

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
            if let Ok(Some(item)) = ws_stream.try_next().await {
                println!("Received new block");
                let block = serde_json::from_str::<ApiBlock>(&item.to_string()).unwrap();
                self.data.lock().unwrap().received_blocks.push(block);
            } else {
                thread::sleep(std::time::Duration::from_secs(10));
            }
        }
    }
}
