use crate::PeerSettings;
use ephemera::configuration::Configuration;
use ephemera::membership::JsonPeerInfo;

//Read config from ~/.ephemera/peers.toml
pub(crate) fn read_peers_config() -> Vec<JsonPeerInfo> {
    let mut peers = vec![];
    let path = Configuration::ephemera_root_dir()
        .unwrap()
        .join("peers.toml");

    let config = std::fs::read_to_string(path).unwrap();

    let mut settings = toml::from_str::<PeerSettings>(&config).unwrap();
    settings.peers.sort_by(|a, b| a.name.cmp(&b.name));

    settings.peers.into_iter().for_each(|setting| {
        peers.push(JsonPeerInfo {
            name: setting.name,
            address: setting.address,
            public_key: setting.public_key,
        });
    });
    println!("Read {:?} peers from config", peers.len());
    peers
}
