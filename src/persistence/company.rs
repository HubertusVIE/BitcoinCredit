use crate::service::company_service::{Company, CompanyKeys};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use super::{file_storage_path, Result};
use async_trait::async_trait;
use futures::future::try_join_all;
use log::error;
use tokio::{
    fs::{create_dir_all, read, read_dir, write},
    task,
};

#[async_trait]
pub trait CompanyStoreApi: Send + Sync {
    /// Checks if the given company exists
    async fn exists(&self, id: &str) -> bool;
    /// Fetches the given company
    async fn get(&self, id: &str) -> Result<Company>;
    /// Returns all companies
    async fn get_all(&self) -> Result<HashMap<String, Company>>;
    /// Inserts the company with the given id
    async fn insert(&self, id: &str, data: &Company) -> Result<()>;
    /// Updates the company with the given id
    async fn update(&self, id: &str, data: &Company) -> Result<()>;
    /// Saves the key pair for the given company id
    async fn save_key_pair(&self, id: &str, key_pair: &CompanyKeys) -> Result<()>;
    /// Gets the key pair for the given company id
    async fn get_key_pair(&self, id: &str) -> Result<CompanyKeys>;
}

#[derive(Clone)]
pub struct FileBasedCompanyStore {
    folder: String,
    data_file: String,
    key_pair_file: String,
}

impl FileBasedCompanyStore {
    pub async fn new(
        data_dir: &str,
        path: &str,
        data_file: &str,
        key_pair_file: &str,
    ) -> Result<Self> {
        let folder = file_storage_path(data_dir, path).await?;
        Ok(Self {
            folder,
            data_file: data_file.to_owned(),
            key_pair_file: key_pair_file.to_owned(),
        })
    }

    pub fn get_path_for_company_id(&self, id: &str) -> PathBuf {
        PathBuf::from(self.folder.as_str()).join(id)
    }

    pub fn get_path_for_company_data(&self, id: &str) -> PathBuf {
        PathBuf::from(self.folder.as_str())
            .join(id)
            .join(&self.data_file)
    }

    pub fn get_path_for_company_key_pair(&self, id: &str) -> PathBuf {
        PathBuf::from(self.folder.as_str())
            .join(id)
            .join(&self.key_pair_file)
    }
}

#[async_trait]
impl CompanyStoreApi for FileBasedCompanyStore {
    async fn exists(&self, id: &str) -> bool {
        let path = self.get_path_for_company_data(id);
        task::spawn_blocking(move || path.exists())
            .await
            .unwrap_or(false)
    }

    async fn get(&self, id: &str) -> Result<Company> {
        let path = self.get_path_for_company_data(id);
        let bytes = read(path).await?;
        let company: Company = serde_json::from_slice(&bytes)?;
        Ok(company)
    }

    async fn get_all(&self) -> Result<HashMap<String, Company>> {
        let folder_path = Path::new(&self.folder);

        let mut dir = read_dir(&folder_path).await?;
        let mut ids = vec![];
        while let Some(entry) = dir.next_entry().await? {
            let metadata = entry.metadata().await?;
            if metadata.is_dir() {
                if let Some(dir_name) = entry.file_name().to_str() {
                    ids.push(dir_name.to_owned());
                }
            }
        }

        let tasks = ids.into_iter().map(|id| async move {
            let result = self.get(&id).await?;
            Ok((id, result)) as Result<(String, Company)>
        });
        let results = try_join_all(tasks).await?;
        let map: HashMap<String, Company> = results.into_iter().collect();
        Ok(map)
    }

    async fn insert(&self, id: &str, data: &Company) -> Result<()> {
        let folder_path = self.get_path_for_company_id(id);
        if !folder_path.exists() {
            create_dir_all(&folder_path).await?;
        }

        let json = serde_json::to_string_pretty(data)?;
        let path = self.get_path_for_company_data(id);
        write(path, &json).await?;
        Ok(())
    }

    async fn update(&self, id: &str, data: &Company) -> Result<()> {
        let folder_path = self.get_path_for_company_id(id);
        if !folder_path.exists() {
            error!("could not find company folder for {id} to update company");
            return Err(super::Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "company folder not found",
            )));
        }

        let json = serde_json::to_string_pretty(data)?;
        let path = self.get_path_for_company_data(id);
        write(path, &json).await?;
        Ok(())
    }

    async fn save_key_pair(&self, id: &str, key_pair: &CompanyKeys) -> Result<()> {
        let path = self.get_path_for_company_key_pair(id);
        let json = serde_json::to_string_pretty(key_pair)?;
        write(path, &json).await?;
        Ok(())
    }

    async fn get_key_pair(&self, id: &str) -> Result<CompanyKeys> {
        let path = self.get_path_for_company_key_pair(id);
        let bytes = read(&path).await?;
        let keys = serde_json::from_slice(&bytes)?;
        Ok(keys)
    }
}
