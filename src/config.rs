use anyhow::Result;
use bitcoin::Network;
use clap::Parser;
use libp2p::multiaddr::Protocol;
use libp2p::Multiaddr;
use std::net::Ipv4Addr;

/// Configuration for the bitcredit application
/// Allows to set the ports and addresses for the http and p2p connections
/// either via command line or environment variables
#[derive(Parser, Clone)]
#[command(version, about, long_about = None)]
pub struct Config {
    #[arg(default_value_t = 1908, long, env = "P2P_PORT")]
    pub p2p_port: u16,
    #[arg(default_value_t = String::from("0.0.0.0"), long, env = "P2P_ADDRESS")]
    pub p2p_address: String,
    #[arg(default_value_t = 8000, long, env = "HTTP_PORT")]
    pub http_port: u16,
    #[arg(default_value_t = String::from("127.0.0.1"), long, env = "HTTP_ADDRESS")]
    pub http_address: String,
    #[arg(default_value_t = String::from("."), long, env = "DATA_DIR")]
    pub data_dir: String,
    #[arg(default_value_t = String::from("ws://localhost:8800"), long, env = "SURREAL_DB_CONNECTION")]
    pub surreal_db_connection: String,
    #[arg(default_value_t = false, long, env = "TERMINAL_CLIENT")]
    pub terminal_client: bool,
    #[arg(default_value_t = String::from("development"),  env = "development")]
    pub environment: String,
    #[arg(default_value_t = String::from("ws://localhost:8080"), long, env = "NOSTR_RELAY")]
    pub nostr_relay: String,
    #[arg(default_value_t = String::from("http://127.0.0.1:3338"), long, env = "MINT_URL")]
    pub mint_url: String,
    #[arg(default_value_t = 1, long, env = "JOB_RUNNER_INITIAL_DELAY_SECONDS")]
    pub job_runner_initial_delay_seconds: u64,
    #[arg(default_value_t = 600, long, env = "JOB_RUNNER_CHECK_INTERVAL_SECONDS")]
    pub job_runner_check_interval_seconds: u64,
}

impl Config {
    pub fn http_listen_url(&self) -> String {
        format!("http://{}:{}", self.http_address, self.http_port)
    }

    pub fn p2p_listen_url(&self) -> Result<Multiaddr> {
        let res = Multiaddr::empty()
            .with(self.p2p_address.parse::<Ipv4Addr>()?.into())
            .with(Protocol::Tcp(self.p2p_port));
        Ok(res)
    }

    pub fn bitcoin_network(&self) -> Network {
        match self.environment.as_str() {
            "production" => Network::Bitcoin,
            "development" => Network::Testnet,
            _ => Network::Testnet,
        }
    }
}
