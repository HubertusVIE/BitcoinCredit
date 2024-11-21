use super::{file_storage_path, Result};
use crate::util::file::is_not_hidden_or_directory_async;
use async_trait::async_trait;
use std::path::Path;
use tokio::fs::{create_dir_all, read, read_dir, remove_dir_all, write};

#[cfg(test)]
use mockall::automock;

#[cfg_attr(test, automock)]
#[async_trait]
pub trait FileUploadStoreApi: Send + Sync {
    /// Creates temporary upload folder with the given name
    async fn create_temp_upload_folder(&self, file_upload_id: &str) -> Result<()>;

    /// Deletes temporary upload folder with the given name
    async fn remove_temp_upload_folder(&self, file_upload_id: &str) -> Result<()>;

    /// Writes the temporary upload file with the given file name and bytes for the given file_upload_id
    async fn write_temp_upload_file(
        &self,
        file_upload_id: &str,
        file_name: &str,
        file_bytes: &[u8],
    ) -> Result<()>;

    /// Reads the temporary files from the given file_upload_id and returns their file name and
    /// bytes
    async fn read_temp_upload_files(&self, file_upload_id: &str) -> Result<Vec<(String, Vec<u8>)>>;
}

#[derive(Clone)]
pub struct FileUploadStore {
    temp_upload_folder: String,
}

impl FileUploadStore {
    pub async fn new(data_dir: &str, files_path: &str, temp_upload_path: &str) -> Result<Self> {
        let temp_upload_folder =
            file_storage_path(&format!("{data_dir}/{files_path}"), temp_upload_path).await?;
        Ok(Self { temp_upload_folder })
    }

    pub async fn cleanup_temp_uploads(&self) -> Result<()> {
        log::info!("cleaning up temp upload folder");
        let path = Path::new(&self.temp_upload_folder);
        let mut dir = read_dir(path).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                log::info!("deleting temp upload folder at {path:?}");
                remove_dir_all(path).await?;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl FileUploadStoreApi for FileUploadStore {
    async fn create_temp_upload_folder(&self, file_upload_id: &str) -> Result<()> {
        let dest_dir = Path::new(&self.temp_upload_folder).join(file_upload_id);
        if !dest_dir.exists() {
            create_dir_all(&dest_dir).await?;
        }
        Ok(())
    }

    async fn remove_temp_upload_folder(&self, file_upload_id: &str) -> Result<()> {
        let dest_dir = Path::new(&self.temp_upload_folder).join(file_upload_id);
        if dest_dir.exists() {
            log::info!("deleting temp upload folder for bill at {dest_dir:?}");
            remove_dir_all(dest_dir).await?;
        }
        Ok(())
    }

    async fn write_temp_upload_file(
        &self,
        file_upload_id: &str,
        file_name: &str,
        file_bytes: &[u8],
    ) -> Result<()> {
        let dest = Path::new(&self.temp_upload_folder)
            .join(file_upload_id)
            .join(file_name);
        write(dest, file_bytes).await?;
        Ok(())
    }

    async fn read_temp_upload_files(&self, file_upload_id: &str) -> Result<Vec<(String, Vec<u8>)>> {
        let mut files = Vec::new();
        let folder = Path::new(&self.temp_upload_folder).join(file_upload_id);
        let mut dir = read_dir(&folder).await?;
        while let Some(entry) = dir.next_entry().await? {
            if is_not_hidden_or_directory_async(&entry).await {
                let file_path = entry.path();
                if let Some(file_name) = file_path.file_name() {
                    if let Some(file_name_str) = file_name.to_str() {
                        let file_bytes = read(&file_path).await?;
                        files.push((file_name_str.to_owned(), file_bytes));
                    }
                }
            }
        }
        Ok(files)
    }
}
