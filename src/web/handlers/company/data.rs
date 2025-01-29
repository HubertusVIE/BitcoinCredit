use crate::{
    service::contact_service::{Contact, ContactType},
    web::data::{File, OptionalPostalAddress, PostalAddress},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateCompanyPayload {
    pub name: String,
    pub country_of_registration: String,
    pub city_of_registration: String,
    #[serde(flatten)]
    pub postal_address: PostalAddress,
    pub email: String,
    pub registration_number: String,
    pub registration_date: String,
    pub proof_of_registration_file_upload_id: Option<String>,
    pub logo_file_upload_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EditCompanyPayload {
    pub id: String,
    pub name: Option<String>,
    pub email: Option<String>,
    #[serde(flatten)]
    pub postal_address: OptionalPostalAddress,
    pub logo_file_upload_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AddSignatoryPayload {
    pub id: String,
    pub signatory_node_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoveSignatoryPayload {
    pub id: String,
    pub signatory_node_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ListSignatoriesResponse {
    pub signatories: Vec<SignatoryResponse>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignatoryResponse {
    #[serde(rename = "type")]
    pub t: ContactType,
    pub node_id: String,
    pub name: String,
    #[serde(flatten)]
    pub postal_address: PostalAddress,
    pub avatar_file: Option<File>,
}

impl From<Contact> for SignatoryResponse {
    fn from(value: Contact) -> Self {
        Self {
            t: value.t,
            node_id: value.node_id,
            name: value.name,
            postal_address: value.postal_address,
            avatar_file: value.avatar_file,
        }
    }
}
