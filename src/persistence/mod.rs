pub mod bill;
pub mod company;
pub mod contact;
pub mod db;
pub mod identity;

use bill::FileBasedBillStore;
use db::{contact::SurrealContactStore, get_surreal_db, SurrealDbConfig};
use identity::FileBasedIdentityStore;
use log::error;
use std::{path::Path, sync::Arc};
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

pub use contact::ContactStoreApi;

use crate::config::Config;

/// Given a base path and a directory path, ensures that the directory
/// exists and returns the full path.
pub async fn file_storage_path(data_dir: &str, path: &str) -> Result<String> {
    let directory = format!("{}/{}", data_dir, path);
    if !Path::new(&directory).exists() {
        tokio::fs::create_dir_all(&directory).await?;
    }
    Ok(directory)
}

/// A container for all persistence related dependencies.
#[derive(Clone)]
pub struct DbContext {
    pub contact_store: Arc<dyn ContactStoreApi>,
    pub bill_store: Arc<dyn bill::BillStoreApi>,
    pub identity_store: Arc<dyn identity::IdentityStoreApi>,
}

/// Creates a new instance of the DbContext with the given SurrealDB configuration.
pub async fn get_db_context(conf: &Config) -> Result<DbContext> {
    let surreal_db_config = SurrealDbConfig::new(&conf.surreal_db_connection);
    let db = get_surreal_db(&surreal_db_config).await?;

    let contact_store = Arc::new(SurrealContactStore::new(db));

    let bill_store = Arc::new(
        FileBasedBillStore::new(
            &conf.data_dir,
            "bills",
            "files",
            "temp_upload",
            "bills_keys",
        )
        .await?,
    );
    if let Err(e) = bill_store.cleanup_temp_uploads().await {
        error!("Error cleaning up temp upload folder for bill: {e}");
    }

    let identity_store = Arc::new(
        FileBasedIdentityStore::new(
            &conf.data_dir,
            "identity",
            "identity",
            "peer_id",
            "ed25519_keys",
        )
        .await?,
    );

    Ok(DbContext {
        contact_store,
        bill_store,
        identity_store,
    })
}
