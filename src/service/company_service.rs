use crate::{persistence::file_upload::FileUploadStoreApi, web::data::File};
use borsh_derive::{self, BorshDeserialize, BorshSerialize};
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::persistence::company::CompanyStoreApi;

use super::Result;

#[async_trait]
pub trait CompanyServiceApi: Send + Sync {}

/// The company service is responsible for managing the companies
#[derive(Clone)]
pub struct CompanyService {
    store: Arc<dyn CompanyStoreApi>,
    file_upload_store: Arc<dyn FileUploadStoreApi>,
}

impl CompanyService {
    pub fn new(
        store: Arc<dyn CompanyStoreApi>,
        file_upload_store: Arc<dyn FileUploadStoreApi>,
    ) -> Self {
        Self {
            store,
            file_upload_store,
        }
    }
}

#[async_trait]
impl CompanyServiceApi for CompanyService {}

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
