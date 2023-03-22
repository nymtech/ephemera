use tokio::sync::mpsc::UnboundedSender;

use ephemera::network::{PeerDiscovery, PeerInfo};

pub(crate) struct HttpPeerDiscovery {
    smart_contract_url: String,
}

impl HttpPeerDiscovery {
    pub(crate) fn new(smart_contract_url: String) -> Self {
        Self { smart_contract_url }
    }
}

#[async_trait::async_trait]
impl PeerDiscovery for HttpPeerDiscovery {
    async fn poll(
        &mut self,
        discovery_channel: UnboundedSender<Vec<PeerInfo>>,
    ) -> anyhow::Result<()> {
        let url = format!("http://{}/contract/peer_info", self.smart_contract_url);
        log::info!("Requesting peers from: {url}");
        match reqwest::get(url).await?.json().await {
            Ok(response) => {
                log::info!("Response: {:?}", response);
                discovery_channel.send(response).unwrap();
            }
            Err(err) => {
                log::error!("Error while parsing response: {}", err);
            }
        }
        Ok(())
    }

    fn get_request_interval_in_sec(&self) -> u64 {
        60
    }
}
