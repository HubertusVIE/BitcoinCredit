pub mod bill;
pub mod contact;
pub mod db;
pub mod identity;

use std::path::Path;
use thiserror::Error;

/// Generic persistence result type
pub type Result<T> = std::result::Result<T, Error>;

/// Generic persistence error type
#[derive(Debug, Error)]
pub enum Error {
    #[error("io error {0}")]
    Io(#[from] std::io::Error),

    #[error("SurrealDB connection error {0}")]
    SurrealConnection(#[from] surrealdb::Error),

    #[error("unable to serialize/deserialize to/from JSON {0}")]
    Json(#[from] serde_json::Error),

    #[error("unable to serialize/deserialize PeerId {0}")]
    PeerId(#[from] libp2p::multihash::Error),

    #[error("unable to serialize/deserialize Keypair {0}")]
    Keypair(#[from] libp2p::identity::DecodingError),

    #[error("no such {0} entity {1}")]
    NoSuchEntity(String, String),
}

pub use contact::{ContactStoreApi, FileBasedContactStore};

/// Given a base path and a directory path, ensures that the directory
/// exists and returns the full path.
pub async fn file_storage_path(data_dir: &str, path: &str) -> Result<String> {
    let directory = format!("{}/{}", data_dir, path);
    if !Path::new(&directory).exists() {
        tokio::fs::create_dir_all(&directory).await?;
    }
    Ok(directory)
}
