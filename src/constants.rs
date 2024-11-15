use bitcoin::Network;
use std::net::Ipv4Addr;

// General
pub const BILLS_PREFIX: &str = "BILLS";
pub const BILL_PREFIX: &str = "BILL";
pub const BILL_ATTACHMENT_PREFIX: &str = "BILLATT";
pub const KEY_PREFIX: &str = "KEY";
pub const SHUTDOWN_GRACE_PERIOD_MS: u64 = 1500;

// Validation
pub const MAX_FILE_SIZE_BYTES: usize = 5_000_000; // ~5 MB
pub const MAX_FILE_NAME_CHARACTERS: usize = 50;
pub const VALID_FILE_MIME_TYPES: [&str; 3] = ["image/jpeg", "image/png", "application/pdf"];

// Paths
pub const BILLS_FOLDER_PATH: &str = "bills";
pub const BILLS_KEYS_FOLDER_PATH: &str = "bills_keys";
pub const QUOTES_MAP_FOLDER_PATH: &str = "quotes";
pub const BOOTSTRAP_FOLDER_PATH: &str = "bootstrap";
pub const IDENTITY_PEER_ID_FILE_PATH: &str = "identity/peer_id";
pub const QUOTE_MAP_FILE_PATH: &str = "quotes/quotes";
pub const BOOTSTRAP_NODES_FILE_PATH: &str = "bootstrap/bootstrap_nodes.json";

// Bitcoin
pub const USEDNET: Network = Network::Testnet; // use Network::Bitcoin for Mainnet
pub const COMPOUNDING_INTEREST_RATE_ZERO: u64 = 0;
// pub const BILL_VALIDITY_PERIOD: u64 = 90;

// Relay
pub const RELAY_BOOTSTRAP_NODE_ONE_IP: Ipv4Addr = Ipv4Addr::new(45, 147, 248, 87);
pub const RELAY_BOOTSTRAP_NODE_ONE_TCP: u16 = 1908;
pub const RELAY_BOOTSTRAP_NODE_ONE_PEER_ID: &str =
    "12D3KooWFvRxAazxdKVB7SsTtcLTnvmF8brtW2kQRhceohtgcJv2";
