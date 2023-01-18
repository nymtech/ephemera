use crate::broadcast_callback::SignaturesConsensusRequest;
use ephemera::broadcast_protocol::websocket::wsmanager::WsManagerHandle;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsSignaturesMsg {
    pub request: SignaturesConsensusRequest,
}

pub struct WsBackend {
    websocket: WsManagerHandle,
}

impl WsBackend {
    pub fn new(ws_manager: WsManagerHandle) -> WsBackend {
        WsBackend {
            websocket: ws_manager,
        }
    }

    pub async fn send(&mut self, sig_msg: Vec<u8>) -> Result<()> {
        self.websocket.send(sig_msg).await?;
        Ok(())
    }
}
