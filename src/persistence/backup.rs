use super::Result;
use async_trait::async_trait;

#[cfg(test)]
use mockall::automock;

/// Backup and restore the database from/to bytes.
#[cfg_attr(test, automock)]
#[async_trait]
pub trait BackupStoreApi: Send + Sync {
    /// creates a backup of the currently active database as a byte vector
    /// ready for encryption
    async fn backup(&self) -> Result<Vec<u8>>;
}
