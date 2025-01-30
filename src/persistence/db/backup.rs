use crate::persistence::backup::BackupStoreApi;

use super::Result;
use async_trait::async_trait;
use futures::StreamExt;
use surrealdb::{engine::any::Any, Surreal};

pub struct SurrealBackupStore {
    db: Surreal<Any>,
}

impl SurrealBackupStore {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl BackupStoreApi for SurrealBackupStore {
    /// create a file backup of the database and returns the file name
    async fn backup(&self) -> Result<Vec<u8>> {
        let mut stream = self.db.export(()).await?;
        let mut buffer = Vec::new();
        while let Some(Ok(chunk)) = stream.next().await {
            buffer.extend_from_slice(&chunk);
        }
        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use crate::persistence::db::get_memory_db;

    use super::*;

    #[tokio::test]
    async fn test_backup() {
        let db = get_memory_db("test", "backup")
            .await
            .expect("could not create get_memory_db");

        let store = SurrealBackupStore::new(db);
        let result = store.backup().await;
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }
}
