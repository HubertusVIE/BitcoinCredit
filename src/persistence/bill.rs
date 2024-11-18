use super::{file_storage_path, Result};
use crate::{bill::BillKeys, blockchain::Chain};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::{
    fs::{self, create_dir_all, read, write, File},
    io::AsyncReadExt,
};

#[cfg(test)]
use mockall::automock;

#[cfg_attr(test, automock)]
#[async_trait]
pub trait BillStoreApi: Send + Sync {
    /// Reads the blockchain of the given bill from disk
    async fn read_bill_chain_from_file(&self, bill_name: &str) -> Result<Chain>;

    /// Writes the given encrypted bytes of an attached file to disk
    async fn save_attached_file(
        &self,
        encrypted_bytes: &[u8],
        bill_name: &str,
        file_name: &str,
    ) -> Result<()>;

    /// Opens the given attached file from disk
    async fn open_attached_file(&self, bill_name: &str, file_name: &str) -> Result<Vec<u8>>;

    /// Writes bill keys to file
    async fn write_bill_keys_to_file(
        &self,
        bill_name: String,
        private_key: String,
        public_key: String,
    ) -> Result<()>;

    /// Writes the given pretty printed chain as JSON to a file
    async fn write_blockchain_to_file(
        &self,
        bill_name: &str,
        pretty_printed_chain_as_json: String,
    ) -> Result<()>;

    /// Reads bill keys from file
    async fn read_bill_keys_from_file(&self, bill_name: &str) -> Result<BillKeys>;
}

#[derive(Clone)]
pub struct FileBasedBillStore {
    folder: String,
    files_folder: String,
    keys_folder: String,
}

impl FileBasedBillStore {
    pub async fn new(
        data_dir: &str,
        path: &str,
        files_path: &str,
        keys_path: &str,
    ) -> Result<Self> {
        let folder = file_storage_path(data_dir, path).await?;
        let files_folder = file_storage_path(&format!("{data_dir}/{files_path}"), path).await?;
        let keys_folder = file_storage_path(data_dir, keys_path).await?;
        Ok(Self {
            folder,
            files_folder,
            keys_folder,
        })
    }

    pub fn get_path_for_bill(&self, bill_name: &str) -> PathBuf {
        let mut path = PathBuf::from(self.folder.as_str()).join(bill_name);
        path.set_extension("json");
        path
    }

    pub fn get_path_for_bill_keys(&self, key_name: &str) -> PathBuf {
        let mut path = PathBuf::from(self.keys_folder.as_str()).join(key_name);
        path.set_extension("json");
        path
    }
}

#[async_trait]
impl BillStoreApi for FileBasedBillStore {
    async fn read_bill_chain_from_file(&self, bill_name: &str) -> Result<Chain> {
        let path = self.get_path_for_bill(bill_name);
        let bytes = read(path).await.map_err(super::Error::Io)?;
        serde_json::from_slice(&bytes).map_err(super::Error::Json)
    }

    async fn save_attached_file(
        &self,
        encrypted_bytes: &[u8],
        bill_name: &str,
        file_name: &str,
    ) -> Result<()> {
        let dest_dir = Path::new(&self.files_folder).join(bill_name);
        if !dest_dir.exists() {
            create_dir_all(&dest_dir).await.map_err(super::Error::Io)?;
        }
        let dest_file = dest_dir.join(file_name);
        write(dest_file, encrypted_bytes)
            .await
            .map_err(super::Error::Io)
    }

    async fn open_attached_file(&self, bill_name: &str, file_name: &str) -> Result<Vec<u8>> {
        let folder = Path::new(&self.files_folder)
            .join(bill_name)
            .join(file_name);

        let mut file = File::open(&folder).await.map_err(super::Error::Io)?;
        let mut buf = Vec::new();

        file.read_to_end(&mut buf).await.map_err(super::Error::Io)?;
        Ok(buf)
    }

    async fn write_bill_keys_to_file(
        &self,
        bill_name: String,
        private_key: String,
        public_key: String,
    ) -> Result<()> {
        let keys: BillKeys = BillKeys {
            private_key_pem: private_key,
            public_key_pem: public_key,
        };

        let output_path = self.get_path_for_bill_keys(&bill_name);
        fs::write(
            output_path,
            serde_json::to_string_pretty(&keys).map_err(super::Error::Json)?,
        )
        .await?;
        Ok(())
    }

    async fn write_blockchain_to_file(
        &self,
        bill_name: &str,
        pretty_printed_chain_as_json: String,
    ) -> Result<()> {
        let path = self.get_path_for_bill(bill_name);
        fs::write(path, pretty_printed_chain_as_json).await?;
        Ok(())
    }

    async fn read_bill_keys_from_file(&self, bill_name: &str) -> Result<BillKeys> {
        let input_path = self.get_path_for_bill_keys(bill_name);
        let bytes = read(&input_path).await.map_err(super::Error::Io)?;
        serde_json::from_slice(&bytes).map_err(super::Error::Json)
    }
}
