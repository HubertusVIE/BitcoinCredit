use std::str::FromStr;

use super::{base58_decode, base58_encode};
use bitcoin::{
    secp256k1::{self, schnorr::Signature, Keypair, Scalar, SecretKey},
    Network,
};
use nostr_sdk::ToBech32;
use secp256k1::{rand, Message, PublicKey, Secp256k1};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Private key error: {0}")]
    PrivateKey(#[from] secp256k1::Error),

    #[error("Nostr key error: {0}")]
    NostrKey(#[from] nostr_sdk::key::Error),

    #[error("Nostr bech32 key error: {0}")]
    NostrNip19(#[from] nostr_sdk::nips::nip19::Error),

    #[error("Libp2p key conversion error: {0}")]
    Secp256k1Conversion(#[from] libp2p::identity::DecodingError),

    #[error("Libp2p key error: {0}")]
    LibP2p(#[from] libp2p::identity::OtherVariantError),

    #[error("Signature had invalid length")]
    InvalidSignatureLength,

    #[error("Aggregated signature needs at least 2 keys")]
    TooFewKeys,

    /// Errors stemming from decoding base58
    #[error("Decode base58 error: {0}")]
    Decode(#[from] super::Error),

    /// Errors stemming from parsing the recovery id
    #[error("Parse recovery id error: {0}")]
    ParseRecoveryId(#[from] std::num::ParseIntError),
}

/// A wrapper around the secp256k1 keypair that can be used for
/// Bitcoin and Nostr keys.
#[derive(Clone, Debug)]
pub struct BcrKeys {
    inner: Keypair,
}

#[allow(dead_code)]
impl BcrKeys {
    /// Generates a fresh random keypair that can be used for
    /// Bitcoin and Nostr keys.
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

    pub fn get_bitcoin_keys(
        &self,
        used_network: Network,
    ) -> (bitcoin::PrivateKey, bitcoin::PublicKey) {
        let secp = Secp256k1::new();
        let private_key = self.get_bitcoin_private_key(used_network);
        (private_key, private_key.public_key(&secp))
    }

    /// Returns the key pair as a bitcoin private key for the given network
    pub fn get_bitcoin_private_key(&self, used_network: Network) -> bitcoin::PrivateKey {
        bitcoin::PrivateKey::new(self.inner.secret_key(), used_network)
    }

    /// Returns the key pair as a nostr key pair
    pub fn get_nostr_keys(&self) -> nostr_sdk::Keys {
        nostr_sdk::Keys::new(self.inner.secret_key().into())
    }

    /// Returns the nostr public key as a bech32 string
    pub fn get_nostr_npub(&self) -> Result<String> {
        Ok(self.get_nostr_keys().public_key().to_bech32()?)
    }

    /// Returns the nostr private key as a bech32 string
    pub fn get_nostr_npriv(&self) -> Result<String> {
        Ok(self.get_nostr_keys().secret_key().to_bech32()?)
    }

    /// Converts the keypair to a libp2p keypair
    pub fn get_libp2p_keys(&self) -> Result<libp2p::identity::Keypair> {
        as_libp2p_keypair(&self.inner)
    }
}

/// libp2p uses a different secp256k1 library than bitcoin and nostr. This
/// function converts the key via priv key bytes and returns the keypair as a
/// libp2p keypair.
fn as_libp2p_keypair(keypair: &Keypair) -> Result<libp2p::identity::Keypair> {
    let secret: libp2p::identity::secp256k1::SecretKey =
        libp2p::identity::secp256k1::SecretKey::try_from_bytes(
            keypair.secret_key().secret_bytes(),
        )?;

    Ok(libp2p::identity::Keypair::from(
        libp2p::identity::secp256k1::Keypair::from(secret),
    ))
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

/// Returns the aggregated public key for the given private keys
pub fn get_aggregated_public_key(private_keys: &[String]) -> Result<String> {
    if private_keys.len() < 2 {
        return Err(Error::TooFewKeys);
    }

    let key_pairs: Vec<Keypair> = private_keys
        .iter()
        .map(|pk| load_keypair(pk))
        .collect::<Result<Vec<Keypair>>>()?;
    let public_keys: Vec<PublicKey> = key_pairs.into_iter().map(|kp| kp.public_key()).collect();

    let first_key = public_keys.first().ok_or(Error::TooFewKeys)?;
    let mut aggregated_key: PublicKey = first_key.to_owned();
    for key in public_keys.iter().skip(1) {
        aggregated_key = aggregated_key.combine(key)?;
    }
    Ok(aggregated_key.to_string())
}

/// The keys need to be in the correct order (identity -> company -> bill) to get the same
/// signature for the same keys
/// Public keys can be aggregated regardless of order
/// Returns the aggregated signature and the aggregated public key
pub fn aggregated_signature(hash: &str, keys: &[String]) -> Result<String> {
    if keys.len() < 2 {
        return Err(Error::TooFewKeys);
    }
    let secp = Secp256k1::signing_only();
    let key_pairs: Vec<Keypair> = keys
        .iter()
        .map(|pk| load_keypair(pk))
        .collect::<Result<Vec<Keypair>>>()?;
    let secret_keys: Vec<SecretKey> = key_pairs.into_iter().map(|kp| kp.secret_key()).collect();

    let first_key = secret_keys.first().ok_or(Error::TooFewKeys)?;

    let mut aggregated_key: SecretKey = first_key.to_owned();
    for key in secret_keys.iter().skip(1) {
        aggregated_key = aggregated_key.add_tweak(&Scalar::from(*key))?;
    }

    let aggregated_key_pair = Keypair::from_secret_key(&secp, &aggregated_key);
    let msg = Message::from_digest_slice(&base58_decode(hash)?)?;
    let signature = secp.sign_schnorr(&msg, &aggregated_key_pair);

    Ok(base58_encode(&signature.serialize()))
}

pub fn signature(hash: &str, private_key: &str) -> Result<String> {
    // create a signing context
    let secp = Secp256k1::signing_only();
    let key_pair = load_keypair(private_key)?;
    let msg = Message::from_digest_slice(&base58_decode(hash)?)?;
    let signature = secp.sign_schnorr(&msg, &key_pair);
    Ok(base58_encode(&signature.serialize()))
}

pub fn verify(hash: &str, signature: &str, public_key: &str) -> Result<bool> {
    // create a verification context
    let secp = Secp256k1::verification_only();
    let (pub_key, _) = PublicKey::from_str(public_key)?.x_only_public_key();
    let msg = Message::from_digest_slice(&base58_decode(hash)?)?;
    let decoded_signature = Signature::from_slice(&base58_decode(signature)?)?;
    Ok(secp
        .verify_schnorr(&decoded_signature, &msg, &pub_key)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util;

    const PKEY: &str = "926a7ce0fdacad199307bcbbcda4869bca84d54b939011bafe6a83cb194130d3";

    #[test]
    fn test_sign_verify() {
        let keypair = BcrKeys::new();
        let hash = util::sha256_hash("Hello, World".as_bytes());
        let signature = signature(&hash, &keypair.get_private_key_string()).unwrap();
        assert!(verify(&hash, &signature, &keypair.get_public_key()).is_ok());
        assert!(verify(&hash, &signature, &keypair.get_public_key()).unwrap());
    }

    #[test]
    fn test_sign_verify_invalid() {
        let keypair = BcrKeys::new();
        let hash = util::sha256_hash("Hello, World".as_bytes());
        let signature = signature(&hash, &keypair.get_private_key_string()).unwrap();
        let hash2 = util::sha256_hash("Hello, Changed Changed Changed World".as_bytes());
        assert!(verify(&hash, &signature, &keypair.get_public_key()).is_ok());
        assert!(verify(&hash, &signature, &keypair.get_public_key()).is_ok());
        // it fails for a different hash
        assert!(verify(&hash2, &signature, &keypair.get_public_key()).is_ok());
        assert!(!verify(&hash2, &signature, &keypair.get_public_key())
            .as_ref()
            .unwrap());
    }

    #[test]
    fn test_sign_verify_aggregated() {
        let keypair1 = BcrKeys::new();
        let keypair2 = BcrKeys::new();
        let keys: Vec<String> = vec![
            keypair1.get_private_key_string(),
            keypair2.get_private_key_string(),
        ];
        let hash = util::sha256_hash("Hello, World".as_bytes());

        let public_key = get_aggregated_public_key(&keys).unwrap();
        let signature = aggregated_signature(&hash, &keys).unwrap();

        assert!(verify(&hash, &signature, &public_key).is_ok());
        assert!(verify(&hash, &signature, &public_key).unwrap());
    }

    #[test]
    fn test_sign_verify_aggregated_order_dependence() {
        let hash = util::sha256_hash("Hello, World".as_bytes());

        let keypair1 = BcrKeys::new();
        let keypair2 = BcrKeys::new();

        let keys: Vec<String> = vec![
            keypair1.get_private_key_string(),
            keypair2.get_private_key_string(),
        ];
        let public_key = get_aggregated_public_key(&keys).unwrap();
        let signature = aggregated_signature(&hash, &keys).unwrap();

        let keys2: Vec<String> = vec![
            keypair2.get_private_key_string(),
            keypair1.get_private_key_string(),
        ];
        let public_key2 = get_aggregated_public_key(&keys2).unwrap();
        let signature2 = aggregated_signature(&hash, &keys2).unwrap();

        assert_ne!(signature, signature2); // the signatures aren't the same
        assert_eq!(public_key, public_key2); // but the public keys are

        assert!(verify(&hash, &signature, &public_key).is_ok());
        assert!(verify(&hash, &signature, &public_key).unwrap());

        assert!(verify(&hash, &signature2, &public_key2).is_ok());
        assert!(verify(&hash, &signature2, &public_key2).unwrap());

        assert!(verify(&hash, &signature, &public_key2).is_ok());
        assert!(verify(&hash, &signature, &public_key2).unwrap());

        assert!(verify(&hash, &signature2, &public_key).is_ok());
        assert!(verify(&hash, &signature2, &public_key).unwrap());
    }

    #[test]
    fn test_sign_verify_aggregated_invalid() {
        let keypair1 = BcrKeys::new();
        let keypair2 = BcrKeys::new();
        let keys: Vec<String> = vec![
            keypair1.get_private_key_string(),
            keypair2.get_private_key_string(),
        ];
        let hash = util::sha256_hash("Hello, World".as_bytes());

        let public_key = get_aggregated_public_key(&keys).unwrap();
        let signature = aggregated_signature(&hash, &keys).unwrap();

        let changed_hash = util::sha256_hash("Hello Hello, World".as_bytes());
        assert!(verify(&changed_hash, &signature, &public_key).is_ok());
        assert!(!verify(&changed_hash, &signature, &public_key).unwrap());
    }

    #[test]
    fn test_sign_verify_aggregated_invalid_only_one_pubkey() {
        let keypair1 = BcrKeys::new();
        let keypair2 = BcrKeys::new();
        let keys: Vec<String> = vec![
            keypair1.get_private_key_string(),
            keypair2.get_private_key_string(),
        ];
        let hash = util::sha256_hash("Hello, World".as_bytes());
        let signature = aggregated_signature(&hash, &keys).unwrap();
        assert!(verify(&hash, &signature, &keypair2.get_public_key()).is_ok());
        assert!(!verify(&hash, &signature, &keypair2.get_public_key()).unwrap());
    }

    #[test]
    fn test_sign_verify_aggregated_multiple() {
        let keypair1 = BcrKeys::new();
        let keypair2 = BcrKeys::new();
        let keypair3 = BcrKeys::new();
        let keys: Vec<String> = vec![
            keypair1.get_private_key_string(),
            keypair2.get_private_key_string(),
            keypair3.get_private_key_string(),
        ];
        let hash = util::sha256_hash("Hello, World".as_bytes());
        let public_key = get_aggregated_public_key(&keys).unwrap();
        let signature = aggregated_signature(&hash, &keys).unwrap();
        assert!(verify(&hash, &signature, &public_key).is_ok());
        assert!(verify(&hash, &signature, &public_key).unwrap());
    }

    #[test]
    fn test_sign_verify_aggregated_multiple_manually_combined_pubkeys_for_verify() {
        let keypair1 = BcrKeys::new();
        let keypair2 = BcrKeys::new();
        let keypair3 = BcrKeys::new();
        let keys: Vec<String> = vec![
            keypair1.get_private_key_string(),
            keypair2.get_private_key_string(),
            keypair3.get_private_key_string(),
        ];
        let hash = util::sha256_hash("Hello, World".as_bytes());
        let public_key = get_aggregated_public_key(&keys).unwrap();
        let signature = aggregated_signature(&hash, &keys).unwrap();

        let combined_pub_key = keypair1
            .inner
            .public_key()
            .combine(&keypair2.inner.public_key())
            .unwrap()
            .combine(&keypair3.inner.public_key())
            .unwrap()
            .to_string();
        assert_eq!(public_key, combined_pub_key);
        assert!(verify(&hash, &signature, &combined_pub_key).is_ok());
        assert!(verify(&hash, &signature, &combined_pub_key).unwrap());
    }

    #[test]
    fn test_sign_verify_aggregated_manually_combined_pubkeys_for_verify() {
        let keypair1 = BcrKeys::new();
        let keypair2 = BcrKeys::new();
        let keys: Vec<String> = vec![
            keypair1.get_private_key_string(),
            keypair2.get_private_key_string(),
        ];
        let hash = util::sha256_hash("Hello, World".as_bytes());
        let public_key = get_aggregated_public_key(&keys).unwrap();
        let signature = aggregated_signature(&hash, &keys).unwrap();

        let combined_pub_key = keypair1
            .inner
            .public_key()
            .combine(&keypair2.inner.public_key())
            .unwrap()
            .to_string();
        assert_eq!(public_key, combined_pub_key);
        assert!(verify(&hash, &signature, &combined_pub_key).is_ok());
        assert!(verify(&hash, &signature, &combined_pub_key).unwrap());
    }

    #[test]
    fn test_sign_verify_aggregated_manually_combined_pubkeys_for_verify_different_order_also_works()
    {
        let keypair1 = BcrKeys::new();
        let keypair2 = BcrKeys::new();
        let keys: Vec<String> = vec![
            keypair1.get_private_key_string(),
            keypair2.get_private_key_string(),
        ];
        let hash = util::sha256_hash("Hello, World".as_bytes());
        let public_key = get_aggregated_public_key(&keys).unwrap();
        let signature = aggregated_signature(&hash, &keys).unwrap();

        let combined_pub_key = keypair2
            .inner
            .public_key()
            .combine(&keypair1.inner.public_key())
            .unwrap()
            .to_string();
        assert_eq!(public_key, combined_pub_key);
        assert!(verify(&hash, &signature, &combined_pub_key).is_ok());
        assert!(verify(&hash, &signature, &combined_pub_key).unwrap());
    }

    #[test]
    fn test_sign_verify_aggregated_manually_combined_pubkeys_for_verify_invalid_pubkey() {
        let keypair1 = BcrKeys::new();
        let keypair2 = BcrKeys::new();
        let keypair3 = BcrKeys::new();
        let keys: Vec<String> = vec![
            keypair1.get_private_key_string(),
            keypair2.get_private_key_string(),
        ];
        let hash = util::sha256_hash("Hello, World".as_bytes());
        let public_key = get_aggregated_public_key(&keys).unwrap();
        let signature = aggregated_signature(&hash, &keys).unwrap();

        let combined_pub_key = keypair1
            .inner
            .public_key()
            .combine(&keypair3.inner.public_key())
            .unwrap()
            .to_string();
        assert_ne!(public_key, combined_pub_key);
        assert!(verify(&hash, &signature, &combined_pub_key).is_ok());
        assert!(!verify(&hash, &signature, &combined_pub_key).unwrap());
    }

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

    #[test]
    fn test_convert_keypair() {
        let keypair = BcrKeys::new();
        let lp2p_keypair = keypair
            .get_libp2p_keys()
            .expect("could not convert keypair to libp2p keypair");
        let secp256k1_keypair = as_secp256k1_keypair(&lp2p_keypair)
            .expect("could not convert keypair to secp256k1 keypair");
        assert_eq!(
            keypair.get_private_key_string(),
            secp256k1_keypair.display_secret().to_string()
        );
    }

    /// reverses the conversion of a libp2p keypair to a secp256k1 keypair for testing
    fn as_secp256k1_keypair(keypair: &libp2p::identity::Keypair) -> Result<Keypair> {
        let secret = keypair.to_owned().try_into_secp256k1()?.secret().to_bytes();
        Ok(Keypair::from_secret_key(
            &Secp256k1::new(),
            &SecretKey::from_slice(&secret)?,
        ))
    }
}
