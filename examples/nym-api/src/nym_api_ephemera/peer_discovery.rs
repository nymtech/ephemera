use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use ephemera::crypto::{EphemeraPublicKey, PublicKey};
use ephemera::peer_discovery::{PeerDiscovery, PeerInfo, Result};

// Information about a peer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct NymPeerInfo {
    pub(crate) name: String,
    pub(crate) address: String,
    pub(crate) pub_key: String,
}

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
    async fn poll(&mut self, discovery_channel: UnboundedSender<Vec<PeerInfo>>) -> Result<()> {
        let url = format!("http://{}/contract/peer_info", self.smart_contract_url);
        log::info!("Requesting peers from: {url}");
        let result: Vec<NymPeerInfo> = reqwest::get(url)
            .await
            .map_err(|err| anyhow::anyhow!("Failed to get peers: {err}"))?
            .json()
            .await
            .map_err(|err| anyhow::anyhow!("Failed to parse peers: {err}"))?;

        let peers = result
            .into_iter()
            .flat_map(|peer| {
                let peer_info = PeerInfo {
                    name: peer.name,
                    address: peer.address,
                    pub_key: PublicKey::from_base58(&peer.pub_key)?,
                };
                Ok::<PeerInfo, anyhow::Error>(peer_info)
            })
            .collect();

        log::info!("Sending peers: {peers:?}");
        discovery_channel.send(peers).expect("Failed to send peers");
        Ok(())
    }

    fn get_poll_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(60)
    }
}
