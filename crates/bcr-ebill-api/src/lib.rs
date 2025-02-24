use anyhow::{Result, anyhow};
use bitcoin::Network;
use libp2p::Multiaddr;
use libp2p::multiaddr::Protocol;
use std::net::Ipv4Addr;
use std::sync::OnceLock;

mod blockchain;
mod constants;
pub mod data;
mod dht;
pub mod external;
mod persistence;
pub mod service;
#[cfg(test)]
mod tests;
pub mod util;

pub use blockchain::Block;
pub use blockchain::Blockchain;
pub use dht::GossipsubEvent;
pub use dht::GossipsubEventId;
pub use dht::dht_main;
pub use persistence::DbContext;
pub use persistence::get_db_context;
pub use persistence::notification::NotificationFilter;

#[derive(Debug, Clone)]
pub struct Config {
    pub bitcoin_network: String,
    pub nostr_relay: String,
    pub relay_bootstrap_address: String,
    pub relay_bootstrap_peer_id: String,
    pub surreal_db_connection: String,
    pub data_dir: String,
    pub p2p_address: String,
    pub p2p_port: u16,
}

static CONFIG: OnceLock<Config> = OnceLock::new();

impl Config {
    pub fn p2p_listen_url(&self) -> Result<Multiaddr> {
        let res = Multiaddr::empty()
            .with(self.p2p_address.parse::<Ipv4Addr>()?.into())
            .with(Protocol::Tcp(self.p2p_port));
        Ok(res)
    }

    pub fn bitcoin_network(&self) -> Network {
        match self.bitcoin_network.as_str() {
            "mainnet" => Network::Bitcoin,
            "testnet" => Network::Testnet,
            "regtest" => Network::Regtest,
            _ => Network::Testnet,
        }
    }
}

pub fn init(conf: Config) -> Result<()> {
    CONFIG
        .set(conf)
        .map_err(|e| anyhow!("Could not initialize E-Bill API: {e:?}"))?;
    Ok(())
}

pub fn get_config() -> &'static Config {
    CONFIG.get().expect("E-Bill API is not initialized")
}
