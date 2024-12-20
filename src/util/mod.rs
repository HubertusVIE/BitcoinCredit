pub mod crypto;
pub mod date;
pub mod file;
pub mod numbers_to_words;
pub mod rsa;
pub mod terminal;

pub use crypto::BcrKeys;

use bitcoin::{Network, PrivateKey, PublicKey};
use openssl::sha::sha256;
use thiserror::Error;
use uuid::Uuid;

#[cfg(not(test))]
pub fn get_uuid_v4() -> Uuid {
    Uuid::new_v4()
}

#[cfg(test)]
pub fn get_uuid_v4() -> Uuid {
    use uuid::uuid;
    uuid!("00000000-0000-0000-0000-000000000000")
}

pub fn create_bitcoin_keypair(used_network: Network) -> (PrivateKey, PublicKey) {
    let (private_key, public_key) = BcrKeys::new().get_bitcoin_keys(used_network);
    (private_key, public_key)
}

pub fn sha256_hash(bytes: &[u8]) -> String {
    base58_encode(&sha256(bytes))
}

#[derive(Debug, Error)]
pub enum Error {
    /// Errors stemming base58 decoding
    #[error("Decode base58 error: {0}")]
    Base58(#[from] bs58::decode::Error),
}

pub fn base58_encode(bytes: &[u8]) -> String {
    bs58::encode(bytes).into_string()
}

#[allow(dead_code)]
pub fn base58_decode(input: &str) -> std::result::Result<Vec<u8>, Error> {
    let result = bs58::decode(input).into_vec()?;
    Ok(result)
}
