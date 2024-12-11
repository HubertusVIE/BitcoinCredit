pub mod crypto;
pub mod date;
pub mod file;
pub mod numbers_to_words;
pub mod rsa;
pub mod terminal;
use bitcoin::{Network, PrivateKey, PublicKey};
pub use crypto::BcrKeys;
use openssl::sha::sha256;
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
    hex::encode(sha256(bytes))
}
