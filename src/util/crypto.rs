use std::str::FromStr;

use super::Result;
use bitcoin::{
    secp256k1::{self, Keypair, SecretKey},
    Network,
};
use nostr_sdk::{Keys, ToBech32};
use secp256k1::{rand, Secp256k1};

/// A wrapper around the secp256k1 keypair that can be used for
/// Bitcoin and Nostr keys.
#[derive(Clone, Debug)]
pub struct BcrKeys {
    inner: Keypair,
}

#[allow(dead_code)]
impl BcrKeys {
    /// Generates a fresh random keypair that can be used for
    /// Bitocin and Nostr keys.
    pub fn new() -> Self {
        Self {
            inner: generate_keypair(),
        }
    }

    /// Loads a keypair from a given private key string
    pub fn from_private_key(private_key: &str) -> Result<Self> {
        let keypair = load_keypair(private_key)?;
        Ok(Self { inner: keypair })
    }

    /// Returns the private key as a hex encoded string
    pub fn get_private_key_string(&self) -> String {
        self.inner.secret_key().display_secret().to_string()
    }

    /// Returns the public key as a hex encoded string
    pub fn get_public_key(&self) -> String {
        self.inner.public_key().to_string()
    }

    /// Returns the key pair as a bitcoin private key for the given network
    pub fn get_bitcoin_private_key(&self, used_network: Network) -> bitcoin::PrivateKey {
        bitcoin::PrivateKey::new(self.inner.secret_key(), used_network)
    }

    /// Returns the key pair as a nostr key pair
    pub fn get_nostr_keys(&self) -> nostr_sdk::Keys {
        Keys::new(self.inner.secret_key().into())
    }

    /// Returns the nostr public key as a bech32 string
    pub fn get_nostr_npub(&self) -> Result<String> {
        Ok(self.get_nostr_keys().public_key().to_bech32()?)
    }

    /// Returns the nostr private key as a bech32 string
    pub fn get_nostr_npriv(&self) -> Result<String> {
        Ok(self.get_nostr_keys().secret_key().to_bech32()?)
    }
}

/// Generates a new keypair using the secp256k1 library
fn generate_keypair() -> Keypair {
    let secp = Secp256k1::new();
    Keypair::new(&secp, &mut rand::thread_rng())
}

/// Loads a secp256k1 keypair from a private key string
fn load_keypair(private_key: &str) -> Result<Keypair> {
    let secp = Secp256k1::new();
    let pair = Keypair::from_secret_key(&secp, &SecretKey::from_str(private_key)?);
    Ok(pair)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PKEY: &str = "926a7ce0fdacad199307bcbbcda4869bca84d54b939011bafe6a83cb194130d3";

    #[test]
    fn test_new_keypair() {
        let keypair = BcrKeys::new();
        assert!(!keypair.get_private_key_string().is_empty());
        assert!(!keypair.get_public_key().is_empty());
        assert!(!keypair
            .get_bitcoin_private_key(Network::Bitcoin)
            .to_string()
            .is_empty());
        assert!(keypair.get_nostr_keys().public_key().to_bech32().is_ok());
        assert!(keypair.get_nostr_npriv().is_ok());
    }

    #[test]
    fn test_load_keypair() {
        let keypair = BcrKeys::from_private_key(PKEY).unwrap();
        let keypair2 = BcrKeys::from_private_key(PKEY).unwrap();
        assert_eq!(
            keypair.get_private_key_string(),
            keypair2.get_private_key_string()
        );
        assert_eq!(keypair.get_public_key(), keypair2.get_public_key());
        assert_eq!(
            keypair.get_bitcoin_private_key(Network::Bitcoin),
            keypair2.get_bitcoin_private_key(Network::Bitcoin)
        );
        assert_eq!(keypair.get_nostr_keys(), keypair2.get_nostr_keys());
        assert_eq!(
            keypair.get_nostr_npub().unwrap(),
            keypair2.get_nostr_npub().unwrap()
        );
        assert_eq!(
            keypair.get_nostr_npriv().unwrap(),
            keypair2.get_nostr_npriv().unwrap()
        );
    }
}
