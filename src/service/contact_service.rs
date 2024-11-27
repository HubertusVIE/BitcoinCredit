use borsh_derive::{self, BorshDeserialize, BorshSerialize};
use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
#[cfg(test)]
use mockall::automock;
use serde::{Deserialize, Serialize};

use crate::{dht::Client, persistence::ContactStoreApi, service::identity_service::Identity};

use super::Result;

#[cfg_attr(test, automock)]
#[async_trait]
pub trait ContactServiceApi: Send + Sync {
    /// Returns all contacts in short form
    async fn get_contacts(&self) -> Result<Vec<Contact>>;

    /// Returns the identity by name. Right now it will refresh the dht data on every
    /// call so some sort of caching might be needed in the future.
    async fn get_identity_by_name(&self, name: &str) -> Result<IdentityPublicData>;

    /// Deletes the identity with the given name.
    async fn delete_identity_by_name(&self, name: &str) -> Result<()>;

    /// Updates the name of the identity from the given old name to the new name.
    /// This acts like a primary key update and the entity will only be accessible via
    /// the new name.
    async fn update_identity_name(&self, old_name: &str, new_name: &str) -> Result<()>;

    /// Updates the identity with the given name with the new identity data.
    async fn update_identity(&self, name: &str, identity: IdentityPublicData) -> Result<()>;

    /// Adds a new peer identity to the identity with the given name. The data will be
    /// fetched from the dht. It will be stored with name and peer_id only if no dht entry
    /// exists.
    async fn add_peer_identity(&self, name: &str, peer_id: &str) -> Result<IdentityPublicData>;

    /// Returns whether a given npub is in our contact list.
    async fn is_known_npub(&self, npub: &str) -> Result<bool>;
}

/// The contact service is responsible for managing the contacts and syncing them with the
/// dht data.
#[derive(Clone)]
pub struct ContactService {
    client: Client,
    store: Arc<dyn ContactStoreApi>,
}

impl ContactService {
    pub fn new(client: Client, store: Arc<dyn ContactStoreApi>) -> Self {
        Self { client, store }
    }
}

#[async_trait]
impl ContactServiceApi for ContactService {
    async fn get_contacts(&self) -> Result<Vec<Contact>> {
        let identities = self.store.get_map().await?;
        Ok(as_contacts(identities))
    }

    async fn get_identity_by_name(&self, name: &str) -> Result<IdentityPublicData> {
        if let Some(identity) = self.store.by_name(name).await? {
            let public = self
                .client
                .clone()
                .get_identity_public_data_from_dht(identity.peer_id.clone())
                .await?;

            if !public.name.is_empty() && public.ne(&identity) {
                self.update_identity(name, public.to_owned()).await?;
                Ok(public)
            } else {
                Ok(identity)
            }
        } else {
            Ok(IdentityPublicData::new_empty())
        }
    }

    async fn delete_identity_by_name(&self, name: &str) -> Result<()> {
        self.store.delete(name).await?;
        Ok(())
    }

    async fn update_identity_name(&self, old_name: &str, new_name: &str) -> Result<()> {
        self.store.update_name(old_name, new_name).await?;
        Ok(())
    }

    async fn update_identity(&self, name: &str, identity: IdentityPublicData) -> Result<()> {
        self.store.update(name, identity).await?;
        Ok(())
    }

    async fn add_peer_identity(&self, name: &str, peer_id: &str) -> Result<IdentityPublicData> {
        let default = IdentityPublicData::new_only_peer_id(peer_id.to_owned());
        let public = self
            .client
            .clone()
            .get_identity_public_data_from_dht(peer_id.to_owned())
            .await?;

        if public.name.is_empty() {
            self.store.insert(name, default.clone()).await?;
            Ok(default)
        } else {
            self.store.insert(name, public.clone()).await?;
            Ok(public)
        }
    }

    async fn is_known_npub(&self, npub: &str) -> Result<bool> {
        Ok(self.store.get_by_npub(npub).await?.is_some())
    }
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct Contact {
    pub name: String,
    pub peer_id: String,
}

// converts identity data to contact data
fn as_contacts(identities: HashMap<String, IdentityPublicData>) -> Vec<Contact> {
    identities
        .into_iter()
        .map(|(name, public_data)| Contact {
            name,
            peer_id: public_data.peer_id,
        })
        .collect()
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(crate = "rocket::serde")]
pub struct IdentityPublicData {
    pub peer_id: String,
    pub name: String,
    pub company: String,
    pub bitcoin_public_key: String,
    pub postal_address: String,
    pub email: String,
    pub rsa_public_key_pem: String,
    pub nostr_npub: Option<String>,
    pub nostr_relay: Option<String>,
}

impl IdentityPublicData {
    pub fn new(identity: Identity, peer_id: String) -> Self {
        Self {
            peer_id,
            name: identity.name,
            company: identity.company,
            bitcoin_public_key: identity.bitcoin_public_key,
            postal_address: identity.postal_address,
            email: identity.email,
            rsa_public_key_pem: identity.public_key_pem,
            nostr_npub: identity.nostr_npub,
            nostr_relay: identity.nostr_relay,
        }
    }

    pub fn new_empty() -> Self {
        Self {
            peer_id: "".to_string(),
            name: "".to_string(),
            company: "".to_string(),
            bitcoin_public_key: "".to_string(),
            postal_address: "".to_string(),
            email: "".to_string(),
            rsa_public_key_pem: "".to_string(),
            nostr_npub: None,
            nostr_relay: None,
        }
    }

    pub fn new_only_peer_id(peer_id: String) -> Self {
        Self {
            peer_id,
            name: "".to_string(),
            company: "".to_string(),
            bitcoin_public_key: "".to_string(),
            postal_address: "".to_string(),
            email: "".to_string(),
            rsa_public_key_pem: "".to_string(),
            nostr_npub: None,
            nostr_relay: None,
        }
    }
}
