use crate::{
    constants::USEDNET,
    error,
    persistence::{file_upload::FileUploadStoreApi, identity::IdentityStoreApi, ContactStoreApi},
    util,
    web::data::File,
};
use borsh_derive::{self, BorshDeserialize, BorshSerialize};
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::persistence::company::CompanyStoreApi;

use super::Result;
use log::info;

#[async_trait]
pub trait CompanyServiceApi: Send + Sync {
    /// Get a list of companies
    async fn get_list_of_companies(&self) -> Result<Vec<CompanyToReturn>>;

    /// Get a company by id
    async fn get_company_by_id(&self, id: &str) -> Result<CompanyToReturn>;

    /// Create a new company
    async fn create_company(
        &self,
        legal_name: String,
        country_of_registration: String,
        city_of_registration: String,
        postal_address: String,
        legal_email: String,
        registration_number: String,
        registration_date: String,
        proof_of_registration_file_upload_id: Option<String>,
        logo_file_upload_id: Option<String>,
    ) -> Result<CompanyToReturn>;

    /// Changes the given company fields for the given company, if they are set
    async fn edit_company(
        &self,
        id: &str,
        legal_name: Option<String>,
        legal_email: Option<String>,
        postal_address: Option<String>,
        logo_file_upload_id: Option<String>,
    ) -> Result<()>;

    /// Adds another signatory to the given company
    async fn add_signatory(&self, id: &str, signatory_node_id: String) -> Result<()>;

    /// Removes a signatory from the given company
    async fn remove_signatory(&self, id: &str, signatory_node_id: String) -> Result<()>;

    /// Encrypts and saves the given uploaded file, returning the file name, as well as the hash of
    /// the unencrypted file
    async fn encrypt_and_save_uploaded_file(
        &self,
        file_name: &str,
        file_bytes: &[u8],
        id: &str,
        public_key: &str,
    ) -> Result<File>;

    /// opens and decrypts the attached file from the given company
    async fn open_and_decrypt_file(
        &self,
        id: &str,
        file_name: &str,
        private_key: &str,
    ) -> Result<Vec<u8>>;
}

/// The company service is responsible for managing the companies
#[derive(Clone)]
pub struct CompanyService {
    store: Arc<dyn CompanyStoreApi>,
    file_upload_store: Arc<dyn FileUploadStoreApi>,
    identity_store: Arc<dyn IdentityStoreApi>,
    contact_store: Arc<dyn ContactStoreApi>,
}

impl CompanyService {
    pub fn new(
        store: Arc<dyn CompanyStoreApi>,
        file_upload_store: Arc<dyn FileUploadStoreApi>,
        identity_store: Arc<dyn IdentityStoreApi>,
        contact_store: Arc<dyn ContactStoreApi>,
    ) -> Self {
        Self {
            store,
            file_upload_store,
            identity_store,
            contact_store,
        }
    }

    async fn process_upload_file(
        &self,
        upload_id: &Option<String>,
        id: &str,
        public_key: &str,
    ) -> Result<Option<File>> {
        if let Some(upload_id) = upload_id {
            let files = self
                .file_upload_store
                .read_temp_upload_files(upload_id)
                .await?;
            if !files.is_empty() {
                let (file_name, file_bytes) = &files[0];
                let file = self
                    .encrypt_and_save_uploaded_file(file_name, file_bytes, id, public_key)
                    .await?;
                return Ok(Some(file));
            }
        }
        Ok(None)
    }
}

#[async_trait]
impl CompanyServiceApi for CompanyService {
    async fn get_list_of_companies(&self) -> Result<Vec<CompanyToReturn>> {
        let results = self.store.get_all().await?;
        let companies: Vec<CompanyToReturn> = results
            .into_iter()
            .map(|(id, (company, keys))| CompanyToReturn::from(id, company, keys))
            .collect();
        Ok(companies)
    }

    async fn get_company_by_id(&self, id: &str) -> Result<CompanyToReturn> {
        let company = self.store.get(id).await?;
        let keys = self.store.get_key_pair(id).await?;
        Ok(CompanyToReturn::from(id.to_owned(), company, keys))
    }

    async fn create_company(
        &self,
        legal_name: String,
        country_of_registration: String,
        city_of_registration: String,
        postal_address: String,
        legal_email: String,
        registration_number: String,
        registration_date: String,
        proof_of_registration_file_upload_id: Option<String>,
        logo_file_upload_id: Option<String>,
    ) -> Result<CompanyToReturn> {
        let (private_key, public_key) = util::create_bitcoin_keypair(USEDNET);
        let id = util::sha256_hash(&public_key.to_bytes());

        let company_keys = CompanyKeys {
            private_key: private_key.to_string(),
            public_key: public_key.to_string(),
        };

        let identity = self.identity_store.get().await?;
        let peer_id = self.identity_store.get_peer_id().await?;

        let proof_of_registration_file = self
            .process_upload_file(
                &proof_of_registration_file_upload_id,
                &id,
                &identity.public_key_pem,
            )
            .await?;

        let logo_file = self
            .process_upload_file(&logo_file_upload_id, &id, &identity.public_key_pem)
            .await?;

        self.store.save_key_pair(&id, &company_keys).await?;
        let company = Company {
            legal_name,
            country_of_registration,
            city_of_registration,
            postal_address,
            legal_email,
            registration_number,
            registration_date,
            proof_of_registration_file,
            logo_file,
            signatories: vec![peer_id.to_string()], // add caller as signatory
        };
        self.store.insert(&id, &company).await?;

        // clean up temporary file uploads, if there are any, logging any errors
        for upload_id in [proof_of_registration_file_upload_id, logo_file_upload_id]
            .iter()
            .flatten()
        {
            if let Err(e) = self
                .file_upload_store
                .remove_temp_upload_folder(upload_id)
                .await
            {
                error!("Error while cleaning up temporary file uploads for {upload_id}: {e}");
            }
        }

        Ok(CompanyToReturn::from(id, company, company_keys))
    }

    async fn edit_company(
        &self,
        id: &str,
        legal_name: Option<String>,
        legal_email: Option<String>,
        postal_address: Option<String>,
        logo_file_upload_id: Option<String>,
    ) -> Result<()> {
        if !self.store.exists(id).await {
            return Err(super::Error::Validation(String::from(
                "No company with id: {id} found",
            )));
        }
        let mut company = self.store.get(id).await?;
        if let Some(legal_name_to_set) = legal_name {
            company.legal_name = legal_name_to_set;
        }
        if let Some(legal_email_to_set) = legal_email {
            company.legal_email = legal_email_to_set;
        }
        if let Some(postal_address_to_set) = postal_address {
            company.postal_address = postal_address_to_set;
        }
        let identity = self.identity_store.get().await?;
        let logo_file = self
            .process_upload_file(&logo_file_upload_id, id, &identity.public_key_pem)
            .await?;
        company.logo_file = logo_file;

        self.store.update(id, &company).await?;

        Ok(())
    }

    async fn add_signatory(&self, id: &str, signatory_node_id: String) -> Result<()> {
        if !self.store.exists(id).await {
            return Err(super::Error::Validation(String::from(
                "No company with id: {id} found.",
            )));
        }
        let contacts = self.contact_store.get_map().await?;
        let is_in_contacts = contacts
            .iter()
            .any(|(_name, identity)| identity.peer_id == signatory_node_id);
        if !is_in_contacts {
            return Err(super::Error::Validation(String::from(
                "Node Id {signatory_node_id} is not in the contacts.",
            )));
        }

        let mut company = self.store.get(id).await?;
        if company.signatories.contains(&signatory_node_id) {
            return Err(super::Error::Validation(String::from(
                "Node Id {signatory_node_id} is already a signatory.",
            )));
        }
        company.signatories.push(signatory_node_id);
        self.store.update(id, &company).await?;

        Ok(())
    }

    async fn remove_signatory(&self, id: &str, signatory_node_id: String) -> Result<()> {
        if !self.store.exists(id).await {
            return Err(super::Error::Validation(String::from(
                "No company with id: {id} found.",
            )));
        }

        let mut company = self.store.get(id).await?;
        if company.signatories.len() == 1 {
            return Err(super::Error::Validation(String::from(
                "Can't remove last signatory.",
            )));
        }
        if !company.signatories.contains(&signatory_node_id) {
            return Err(super::Error::Validation(String::from(
                "Node id {signatory_node_id} is not a signatory.",
            )));
        }

        let peer_id = self.identity_store.get_peer_id().await?;

        company.signatories.retain(|i| i != &signatory_node_id);
        self.store.update(id, &company).await?;

        if peer_id.to_string() == signatory_node_id {
            info!("Removing self from company {id}");
            self.store.remove(id).await?;
        }

        Ok(())
    }

    async fn encrypt_and_save_uploaded_file(
        &self,
        file_name: &str,
        file_bytes: &[u8],
        id: &str,
        public_key: &str,
    ) -> Result<File> {
        let file_hash = util::sha256_hash(file_bytes);
        let encrypted = util::rsa::encrypt_bytes_with_public_key(file_bytes, public_key);
        self.store
            .save_attached_file(&encrypted, id, file_name)
            .await?;
        info!("Saved company file {file_name} with hash {file_hash} for company {id}");
        Ok(File {
            name: file_name.to_owned(),
            hash: file_hash,
        })
    }

    async fn open_and_decrypt_file(
        &self,
        id: &str,
        file_name: &str,
        private_key: &str,
    ) -> Result<Vec<u8>> {
        let read_file = self.store.open_attached_file(id, file_name).await?;
        let decrypted =
            util::rsa::decrypt_bytes_with_private_key(&read_file, private_key.to_owned());
        Ok(decrypted)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct CompanyToReturn {
    pub id: String,
    pub legal_name: String,
    pub country_of_registration: String,
    pub city_of_registration: String,
    pub postal_address: String,
    pub legal_email: String,
    pub registration_number: String,
    pub registration_date: String,
    pub proof_of_registration_file: Option<File>,
    pub logo_file: Option<File>,
    pub signatories: Vec<String>,
    pub public_key: String,
}

impl CompanyToReturn {
    fn from(id: String, company: Company, company_keys: CompanyKeys) -> CompanyToReturn {
        CompanyToReturn {
            id,
            legal_name: company.legal_name,
            country_of_registration: company.country_of_registration,
            city_of_registration: company.city_of_registration,
            postal_address: company.postal_address,
            legal_email: company.legal_email,
            registration_number: company.registration_number,
            registration_date: company.registration_date,
            proof_of_registration_file: company.proof_of_registration_file,
            logo_file: company.logo_file,
            signatories: company.signatories,
            public_key: company_keys.public_key,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct Company {
    pub legal_name: String,
    pub country_of_registration: String,
    pub city_of_registration: String,
    pub postal_address: String,
    pub legal_email: String,
    pub registration_number: String,
    pub registration_date: String,
    pub proof_of_registration_file: Option<File>,
    pub logo_file: Option<File>,
    pub signatories: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CompanyKeys {
    pub private_key: String,
    pub public_key: String,
}
