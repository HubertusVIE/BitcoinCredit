use crate::persistence::backup::BackupStoreApi;

use super::Result;
use async_trait::async_trait;
use futures::StreamExt;
use surrealdb::{engine::any::Any, Surreal};
use tokio::io::AsyncWriteExt;

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
    async fn backup(&self, file_name: String) -> Result<()> {
        let mut stream = self.db.export(()).await.unwrap();
        let mut file = tokio::fs::File::create(file_name).await?;
        while let Some(Ok(chunk)) = stream.next().await {
            println!(
                "Writing chunk to file {}",
                String::from_utf8(chunk.clone()).unwrap()
            );
            file.write_all(&chunk).await?;
        }
        Ok(())
    }
}
