use std::fmt::Display;
use std::io::Write;
use std::path::PathBuf;

use async_trait::async_trait;
use log::{debug};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::crypto::PublicKey;
use crate::network::Address;
use crate::peer::{Peer, PeerId};

/// Information about an Ephemera peer.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeerInfo {
    /// The name of the peer. Can be arbitrary.
    pub name: String,
    /// The address of the peer.
    /// Expected formats:
    /// 1. `<IP>:<PORT>`
    /// 2. `/ip4/<IP>/tcp/<PORT>` - this is the format used by libp2p multiaddr
    pub address: String,
    /// The public key of the peer. It uniquely identifies the peer.
    /// Public key is used to derive the peer id.
    pub pub_key: PublicKey,
}

impl Display for PeerInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "name {}, address {}, public key {}",
            self.name, self.address, self.pub_key
        )
    }
}

impl TryFrom<PeerInfo> for Peer {
    type Error = anyhow::Error;

    fn try_from(value: PeerInfo) -> std::result::Result<Self, Self::Error> {
        let address: Address = value.address.parse()?;
        let public_key = value.pub_key;
        Ok(Self {
            name: value.name,
            address,
            public_key: public_key.clone(),
            peer_id: PeerId::from_public_key(&public_key),
        })
    }
}

#[derive(Error, Debug)]
pub enum MembersProviderError {
    //Just a placeholder for now
    #[error("GeneralError: {0}")]
    GeneralError(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, MembersProviderError>;

/// The MembersProvider trait allows the user to implement their own peers membership source mechanism.
#[async_trait]
pub trait MembersProvider: Send + Sync {
    /// Ephemera will call this method to poll for new peers.
    ///
    /// Implementation of this trait sends current peers to the members_channel using the provided channel.
    ///
    /// # Arguments
    /// * `members_channel` - The channel to send the new peers to.
    async fn poll(
        &mut self,
        members_channel: tokio::sync::mpsc::UnboundedSender<Vec<PeerInfo>>,
    ) -> Result<()>;

    /// Ephemera will call this method to get the interval between each poll.
    ///
    /// # Returns
    /// * `std::time::Duration` - The interval between each poll.
    fn get_poll_interval(&self) -> std::time::Duration;
}

/// A membership provider that does nothing.
/// Might be useful for testing.
pub struct DummyMembersProvider;

#[async_trait]
impl MembersProvider for DummyMembersProvider {
    async fn poll(&mut self, _: tokio::sync::mpsc::UnboundedSender<Vec<PeerInfo>>) -> Result<()> {
        Ok(())
    }

    fn get_poll_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(60 * 60 * 24)
    }
}

#[derive(Error, Debug)]
pub enum ConfigMembersProviderError {
    #[error("ConfigDoesNotExist: '{0}'")]
    DoesNotExist(String),
    #[error("ParsingFailed: {0}")]
    ParsingFailed(#[from] config::ConfigError),
    #[error("TomlError: {0}")]
    TomlError(#[from] toml::ser::Error),
    #[error("IoError: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PeerSetting {
    /// The name of the peer. Can be arbitrary.
    pub name: String,
    /// The address of the peer.
    /// Expected formats:
    /// 1. `<IP>:<PORT>`
    /// 2. `/ip4/<IP>/tcp/<PORT>` - this is the format used by libp2p multiaddr
    pub address: String,
    ///Serialized public key.
    ///
    /// # Converting to string and back example
    ///```
    /// use ephemera::crypto::{EphemeraKeypair, EphemeraPublicKey, Keypair, PublicKey};
    ///
    /// let public_key = Keypair::generate(None).public_key();
    ///
    /// let public_key_str = public_key.to_string();
    ///
    /// let public_key_parsed = public_key_str.parse::<PublicKey>().unwrap();
    ///
    /// assert_eq!(public_key, public_key_parsed);
    /// ```
    pub public_key: String,
}

impl TryFrom<PeerSetting> for PeerInfo {
    type Error = anyhow::Error;

    fn try_from(setting: PeerSetting) -> std::result::Result<Self, Self::Error> {
        let pub_key = setting.public_key.parse::<PublicKey>()?;
        Ok(PeerInfo {
            name: setting.name,
            address: setting.address,
            pub_key,
        })
    }
}

///[MembersProvider] that reads the peers from a toml config file.
///
/// # Configuration example
/// ```toml
/// [[peers]]
/// name = "node1"
/// address = "/ip4/127.0.0.1/tcp/3000"
/// pub_key = "4XTTMEghav9LZThm6opUaHrdGEEYUkrfkakVg4VAetetBZDWJ"
///
/// [[peers]]
/// name = "node2"
/// address = "/ip4/127.0.0.1/tcp/3001"
/// pub_key = "4XTTMFQt2tgNRmwRgEAaGQe2NXygsK6Vr3pkuBfYezhDfoVty"
/// ```
pub struct ConfigMembersProvider {
    reload_interval: std::time::Duration,
    config_location: PathBuf,
}

impl ConfigMembersProvider {
    pub fn init<I: Into<PathBuf>>(
        path: I,
        reload_interval: std::time::Duration,
    ) -> std::result::Result<Self, ConfigMembersProviderError> {
        Ok(Self {
            reload_interval,
            config_location: path.into(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ConfigPeers {
    peers: Vec<PeerSetting>,
}

impl ConfigPeers {
    pub(crate) fn new(peers: Vec<PeerSetting>) -> Self {
        Self { peers }
    }

    pub(crate) fn try_load<I: Into<PathBuf>>(
        path: I,
    ) -> std::result::Result<ConfigPeers, ConfigMembersProviderError> {
        let path = path.into();
        let config = config::Config::builder()
            .add_source(config::File::from(path))
            .build()?;

        config.try_deserialize().map_err(|err| err.into())
    }

    pub(crate) fn try_write<I: Into<PathBuf>>(
        &self,
        path: I,
    ) -> std::result::Result<(), ConfigMembersProviderError> {
        let config = toml::to_string(&self)?;

        let config = format!(
            "#This file is generated by cli and automatically overwritten every time when cli is run\n{config}",
        );

        let mut file = std::fs::File::create(path.into())?;
        file.write_all(config.as_bytes())?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl MembersProvider for ConfigMembersProvider {
    async fn poll(
        &mut self,
        members_provider_ch: tokio::sync::mpsc::UnboundedSender<Vec<PeerInfo>>,
    ) -> Result<()> {
        let config_peers = ConfigPeers::try_load(self.config_location.clone())
            .map_err(|err| anyhow::anyhow!(err))?;

        let peer_info = config_peers
            .peers
            .iter()
            .map(|peer| PeerInfo::try_from(peer.clone()))
            .collect::<anyhow::Result<Vec<PeerInfo>>>()?;

        members_provider_ch.send(peer_info).unwrap();
        Ok(())
    }

    fn get_poll_interval(&self) -> std::time::Duration {
        self.reload_interval
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct JsonPeerInfo {
    /// The name of the peer. Can be arbitrary.
    pub name: String,
    /// The address of the peer. See [PeerInfo] for more details.
    pub address: String,
    ///Serialized public key.
    ///
    /// # Converting to string and back example
    ///```
    /// use ephemera::crypto::{EphemeraKeypair, EphemeraPublicKey, Keypair, PublicKey};
    ///
    /// let public_key = Keypair::generate(None).public_key();
    ///
    /// let public_key_str = public_key.to_string();
    ///
    /// let public_key_parsed = public_key_str.parse::<PublicKey>().unwrap();
    ///
    /// assert_eq!(public_key, public_key_parsed);
    /// ```
    pub public_key: String,
}

impl JsonPeerInfo {
    pub fn new(name: String, address: String, pub_key: String) -> Self {
        Self {
            name,
            address,
            public_key: pub_key,
        }
    }
}

impl TryFrom<JsonPeerInfo> for PeerInfo {
    type Error = anyhow::Error;

    fn try_from(json_peer_info: JsonPeerInfo) -> std::result::Result<Self, Self::Error> {
        let pub_key = json_peer_info.public_key.parse::<PublicKey>()?;
        Ok(PeerInfo {
            name: json_peer_info.name,
            address: json_peer_info.address,
            pub_key,
        })
    }
}

///[MembersProvider] that reads peers from a http endpoint.
///
/// The endpoint must return a json array of [JsonPeerInfo].
/// # Configuration example
/// ```json
/// [
///  {
///     "name": "node1",
///     "address": "/ip4/",
///     "public_key": "4XTTMEghav9LZThm6opUaHrdGEEYUkrfkakVg4VAetetBZDWJ"
///   },
///  {
///     "name": "node2",
///     "address": "/ip4/",
///     "public_key": "4XTTMFQt2tgNRmwRgEAaGQe2NXygsK6Vr3pkuBfYezhDfoVty"
///   }
/// ]
/// ```
pub struct HttpMembersProvider {
    /// The interval at which the peers are reloaded.
    reload_interval: std::time::Duration,
    /// The url of the http endpoint.
    members_url: String,
}

impl HttpMembersProvider {
    pub fn new(members_url: String, reload_interval: std::time::Duration) -> Self {
        Self {
            reload_interval,
            members_url,
        }
    }

    async fn request_peers(&self) -> Result<Vec<PeerInfo>> {
        debug!("Requesting peers from: {:?}", self.members_url);
        let json_peers: Vec<JsonPeerInfo> = reqwest::get(&self.members_url)
            .await
            .map_err(|err| anyhow::anyhow!("Failed to get peers: {err}"))?
            .json()
            .await
            .map_err(|err| anyhow::anyhow!("Failed to parse peers: {err}"))?;

        let peers = json_peers
            .into_iter()
            .map(TryInto::try_into)
            .collect::<anyhow::Result<Vec<PeerInfo>>>()?;

        Ok(peers)
    }
}

#[async_trait::async_trait]
impl MembersProvider for HttpMembersProvider {
    async fn poll(
        &mut self,
        members_provider_ch: tokio::sync::mpsc::UnboundedSender<Vec<PeerInfo>>,
    ) -> Result<()> {
        let peers = self.request_peers().await?;

        debug!("Sending peers: {peers:?}");
        members_provider_ch.send(peers).expect("Failed to send peers");
        Ok(())
    }

    fn get_poll_interval(&self) -> std::time::Duration {
        self.reload_interval
    }
}
