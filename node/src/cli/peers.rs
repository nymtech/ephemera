//! This is used to add a peer to the configuration file. Only used for testing purposes.

use std::collections::HashMap;

use clap::Parser;

use crate::config::{Configuration, PeerSetting};
use crate::crypto::{EphemeraKeypair, EphemeraPublicKey, Keypair};

#[derive(Debug, Clone, Parser)]
pub struct AddPeerCmd {
    #[clap(short, long)]
    name: String,
    #[clap(short, long)]
    address: String,
    #[clap(short, long)]
    pub_key: String,
}

impl AddPeerCmd {
    pub fn execute(self) {
        log::info!("Add peer command executed");
        match Configuration::try_load_from_home_dir(&self.name) {
            Ok(mut configuration) => {
                let duplicate_name = configuration.libp2p.peers.iter().any(|peer| {
                    if peer.name == self.name {
                        return true;
                    }
                    false
                });

                if duplicate_name {
                    log::error!("Peer with name '{}' already exists", self.name);
                    return;
                }

                configuration.libp2p.peers.push(PeerSetting {
                    name: self.name.clone(),
                    address: self.address,
                    pub_key: self.pub_key,
                });

                if let Err(e) = configuration.try_update_root(self.name.as_str()) {
                    log::error!("Error saving configuration: {}", e);
                }
            }
            Err(err) => {
                log::error!("Error loading configuration: {}", err);
            }
        }
    }
}

#[derive(Debug, Clone, Parser)]
pub struct AddLocalPeersCmd;

impl AddLocalPeersCmd {
    pub fn execute(self) {
        let root_path = Configuration::ephemera_root_dir().unwrap();
        let root_dir = std::fs::read_dir(root_path).unwrap();

        let mut configs = HashMap::new();
        root_dir
            .filter(|entry| {
                let path = entry.as_ref().unwrap().path();
                path.is_dir() && path.file_name().unwrap().to_str().unwrap().contains("node")
            })
            .for_each(|entry| {
                let entry = entry.unwrap();
                let path = entry.path();
                let node_dir = path.file_name().unwrap().to_str().unwrap();
                let conf = Configuration::try_load_from_home_dir(node_dir)
                    .unwrap_or_else(|_| panic!("Error loading configuration for node {node_dir}"));
                configs.insert(String::from(node_dir), conf);
            });

        let peer_names = configs
            .keys()
            .map(|name| name.to_string())
            .collect::<Vec<String>>();

        for peer in peer_names {
            let mut conf = configs.get(peer.as_str()).unwrap().clone();
            conf.libp2p.peers = vec![];
            for (node_name, peer_conf) in configs.iter() {
                if *node_name == peer {
                    continue;
                }
                let address = format!("/ip4/{}/tcp/{}", peer_conf.node.ip, peer_conf.libp2p.port);
                let keypair = Keypair::from_base58(peer_conf.node.private_key.as_str()).unwrap();
                let pub_key = keypair.public_key().to_base58();
                conf.libp2p.peers.push(PeerSetting {
                    name: node_name.to_string(),
                    address,

                    pub_key,
                });
            }
            configs.insert(peer.to_string(), conf);
        }

        for (node_name, conf) in configs {
            if let Err(e) = conf.try_update_root(&node_name) {
                log::error!("Error saving configuration: {}", e);
            }
        }
    }
}
