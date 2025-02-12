pub mod crypto;
pub mod currency;
pub mod date;
pub mod file;
pub mod numbers_to_words;
pub mod terminal;

pub use crypto::BcrKeys;

use sha2::{Digest, Sha256};
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

pub fn sha256_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let hash = hasher.finalize();
    base58_encode(&hash)
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

pub fn update_optional_field(
    field_to_update: &mut Option<String>,
    field: &Option<String>,
    changed: &mut bool,
) {
    match field_to_update {
        Some(_) => {
            if let Some(ref field_to_set) = field {
                *field_to_update = Some(field_to_set.clone());
                *changed = true;
            } else {
                *field_to_update = None;
                *changed = true;
            }
        }
        None => {
            if let Some(ref field_to_set) = field {
                *field_to_update = Some(field_to_set.clone());
                *changed = true;
            }
        }
    };
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_optional_field_baseline() {
        let mut field_to_update = Some(String::from("hi"));
        let mut changed = false;
        update_optional_field(
            &mut field_to_update,
            &Some(String::from("hello")),
            &mut changed,
        );
        assert!(changed);
        assert_eq!(Some(String::from("hello")), field_to_update);
    }

    #[test]
    fn update_optional_field_none() {
        let mut field_to_update = None;
        let mut changed = false;
        update_optional_field(&mut field_to_update, &None, &mut changed);
        assert!(!changed);
        assert_eq!(None, field_to_update);
    }

    #[test]
    fn update_optional_field_some_none() {
        let mut field_to_update = Some(String::from("hi"));
        let mut changed = false;
        update_optional_field(&mut field_to_update, &None, &mut changed);
        assert!(changed);
        assert_eq!(None, field_to_update);
    }
}
