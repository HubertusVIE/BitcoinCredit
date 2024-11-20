use std::collections::HashMap;

use super::Result;
use async_trait::async_trait;
use surrealdb::{engine::any::Any, Surreal};

use crate::{persistence::ContactStoreApi, service::contact_service::IdentityPublicData};

#[derive(Clone)]
pub struct SurrealContactStore {
    db: Surreal<Any>,
}

impl SurrealContactStore {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ContactStoreApi for SurrealContactStore {
    async fn get_map(&self) -> Result<HashMap<String, IdentityPublicData>> {
        todo!()
    }

    async fn by_name(&self, name: &str) -> Result<Option<IdentityPublicData>> {
        todo!()
    }

    async fn insert(&self, name: &str, data: IdentityPublicData) -> Result<()> {
        todo!()
    }

    async fn delete(&self, name: &str) -> Result<()> {
        todo!()
    }

    async fn update_name(&self, name: &str, new_name: &str) -> Result<()> {
        todo!()
    }

    async fn update(&self, name: &str, data: IdentityPublicData) -> Result<()> {
        todo!()
    }

    async fn get_by_npub(&self, npub: &str) -> Result<Option<IdentityPublicData>> {
        todo!()
    }
}

pub struct ContactDb {
    pub peer_id: String,
    pub name: Option<String>,
    pub company: Option<String>,
    pub bitcoin_public_key: Option<String>,
    pub postal_address: Option<String>,
    pub email: Option<String>,
    pub rsa_public_key_pem: Option<String>,
    pub nostr_npub: Option<String>,
    pub nostr_relays: Vec<String>,
}
