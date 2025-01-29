use std::sync::Arc;

use crate::persistence::backup::BackupStoreApi;

use super::Result;
#[cfg(test)]
use mockall::automock;

/// Allows to backup and restore the database as an encrypted file.
#[cfg_attr(test, automock)]
#[async_trait::async_trait]
pub trait BackupServiceApi: Send + Sync {
    /// Creates an encrypted backup of the database and returns the
    /// download path to the backup file.
    async fn backup(&self) -> Result<String>;
}

pub struct BackupService {
    store: Arc<dyn BackupStoreApi>,
}

impl BackupService {
    pub fn new(store: Arc<dyn BackupStoreApi>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl BackupServiceApi for BackupService {
    async fn backup(&self) -> Result<String> {
        let path = "/tmp/bitcredit.surql";
        self.store.backup(path.to_string()).await?;
        Ok("path".to_string())
    }
}
