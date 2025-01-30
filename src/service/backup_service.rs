use std::sync::Arc;

use crate::{
    persistence::{backup::BackupStoreApi, db::SurrealDbConfig, identity::IdentityStoreApi},
    util,
};

use super::{Error, Result};
#[cfg(test)]
use mockall::automock;

/// Allows to backup and restore the database as an encrypted file.
#[cfg_attr(test, automock)]
#[async_trait::async_trait]
pub trait BackupServiceApi: Send + Sync {
    /// Creates an encrypted backup of the database and returns the
    /// download path to the backup file.
    async fn backup(&self) -> Result<Vec<u8>>;
}

pub struct BackupService {
    store: Arc<dyn BackupStoreApi>,
    identity_store: Arc<dyn IdentityStoreApi>,
    surreal_db_config: SurrealDbConfig,
}

impl BackupService {
    pub fn new(
        store: Arc<dyn BackupStoreApi>,
        identity_store: Arc<dyn IdentityStoreApi>,
        surreal_db_config: SurrealDbConfig,
    ) -> Self {
        Self {
            store,
            identity_store,
            surreal_db_config,
        }
    }

    fn validate_surreal_db_connection(&self) -> Result<()> {
        let connection = &self.surreal_db_config.connection_string;
        if connection.starts_with("rocksdb")
            || connection.starts_with("mem")
            || connection.starts_with("http")
        {
            return Ok(());
        }
        Err(Error::Validation(format!(
            "SurrealDB connection {} does not support exports",
            connection
        )))
    }
}

#[async_trait::async_trait]
impl BackupServiceApi for BackupService {
    async fn backup(&self) -> Result<Vec<u8>> {
        self.validate_surreal_db_connection()?;
        let public_key = self
            .identity_store
            .get_full()
            .await?
            .key_pair
            .get_public_key();

        let bytes = self.store.backup().await?;
        let encrypted_bytes = util::crypto::encrypt_ecies(&bytes, &public_key)?;
        Ok(encrypted_bytes)
    }
}

#[cfg(test)]
mod tests {
    use util::BcrKeys;

    use crate::{
        persistence::{backup::MockBackupStoreApi, identity::MockIdentityStoreApi},
        service::identity_service::{Identity, IdentityWithAll},
    };

    use super::*;

    #[tokio::test]
    async fn test_backup_with_embedded_db() {
        let mut store = MockBackupStoreApi::new();
        let mut identity_store = MockIdentityStoreApi::new();
        let surreal_db_config = SurrealDbConfig {
            connection_string: "rocksdb://test".to_string(),
            database: "test".to_string(),
            namespace: "test".to_string(),
        };

        identity_store
            .expect_get_full()
            .returning(|| {
                Ok(IdentityWithAll {
                    identity: Identity::new_empty(),
                    key_pair: BcrKeys::new(),
                })
            })
            .once();

        store
            .expect_backup()
            .returning(|| Ok(vec![0, 1, 0, 1, 0, 0, 1, 0]))
            .once();

        let service =
            BackupService::new(Arc::new(store), Arc::new(identity_store), surreal_db_config);

        let result = service.backup().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_backup_with_external_socket_db_should_fail() {
        let mut store = MockBackupStoreApi::new();
        let mut identity_store = MockIdentityStoreApi::new();
        let surreal_db_config = SurrealDbConfig {
            connection_string: "ws://localhost:8000".to_string(),
            database: "test".to_string(),
            namespace: "test".to_string(),
        };

        identity_store.expect_get_full().never();

        store.expect_backup().never();

        let service =
            BackupService::new(Arc::new(store), Arc::new(identity_store), surreal_db_config);

        let result = service.backup().await;
        assert!(result.is_err());
    }
}
