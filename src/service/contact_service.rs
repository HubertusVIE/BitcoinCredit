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

    /// Adds a new node identity to the identity with the given name. The data will be
    /// fetched from the dht. It will be stored with name and node_id only if no dht entry
    /// exists.
    async fn add_node_identity(&self, name: &str, node_id: &str) -> Result<IdentityPublicData>;

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
                .get_identity_public_data_from_dht(identity.node_id.clone())
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

    async fn add_node_identity(&self, name: &str, node_id: &str) -> Result<IdentityPublicData> {
        let default = IdentityPublicData::new_only_node_id(node_id.to_owned());
        let public = self
            .client
            .clone()
            .get_identity_public_data_from_dht(node_id.to_owned())
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
pub struct Contact {
    pub name: String,
    pub node_id: String,
}

// converts identity data to contact data
fn as_contacts(identities: HashMap<String, IdentityPublicData>) -> Vec<Contact> {
    identities
        .into_iter()
        .map(|(name, public_data)| Contact {
            name,
            node_id: public_data.node_id,
        })
        .collect()
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct IdentityPublicData {
    pub node_id: String,
    pub bitcoin_public_key: String,
    pub name: String,
    pub company: String,
    pub postal_address: String,
    pub email: String,
    pub nostr_npub: Option<String>,
    pub nostr_relay: Option<String>,
}

impl IdentityPublicData {
    pub fn new(identity: Identity) -> Self {
        Self {
            node_id: identity.node_id,
            bitcoin_public_key: identity.bitcoin_public_key,
            name: identity.name,
            company: identity.company,
            postal_address: identity.postal_address,
            email: identity.email,
            nostr_npub: identity.nostr_npub,
            nostr_relay: identity.nostr_relay,
        }
    }

    pub fn new_empty() -> Self {
        Self {
            node_id: "".to_string(),
            bitcoin_public_key: "".to_string(),
            name: "".to_string(),
            company: "".to_string(),
            postal_address: "".to_string(),
            email: "".to_string(),
            nostr_npub: None,
            nostr_relay: None,
        }
    }

    pub fn new_only_node_id(node_id: String) -> Self {
        Self {
            node_id,
            bitcoin_public_key: "".to_string(),
            name: "".to_string(),
            company: "".to_string(),
            postal_address: "".to_string(),
            email: "".to_string(),
            nostr_npub: None,
            nostr_relay: None,
        }
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::persistence::{
        bill::MockBillStoreApi,
        company::{MockCompanyChainStoreApi, MockCompanyStoreApi},
        contact::MockContactStoreApi,
        file_upload::MockFileUploadStoreApi,
        identity::MockIdentityStoreApi,
    };
    use futures::channel::mpsc;

    fn get_service(mock_storage: MockContactStoreApi) -> ContactService {
        let (sender, _) = mpsc::channel(0);
        ContactService::new(
            Client::new(
                sender,
                Arc::new(MockBillStoreApi::new()),
                Arc::new(MockCompanyStoreApi::new()),
                Arc::new(MockCompanyChainStoreApi::new()),
                Arc::new(MockIdentityStoreApi::new()),
                Arc::new(MockFileUploadStoreApi::new()),
            ),
            Arc::new(mock_storage),
        )
    }

    #[tokio::test]
    async fn get_contacts_baseline() {
        let mut store = MockContactStoreApi::new();
        store.expect_get_map().returning(|| {
            let mut identity = IdentityPublicData::new_empty();
            identity.name = "Minka".to_string();
            identity.node_id = "some_node_id".to_string();
            let mut map = HashMap::new();
            map.insert("Minka".to_string(), identity);
            Ok(map)
        });
        let result = get_service(store).get_contacts().await;
        assert!(result.is_ok());
        assert_eq!(result.as_ref().unwrap().first().unwrap().name, *"Minka");
        assert_eq!(
            result.as_ref().unwrap().first().unwrap().node_id,
            *"some_node_id"
        );
    }

    #[tokio::test]
    async fn get_identity_by_name_baseline() {
        let mut store = MockContactStoreApi::new();
        store.expect_by_name().returning(|_| {
            let mut identity = IdentityPublicData::new_empty();
            identity.name = "Minka".to_string();
            identity.node_id = "some_node_id".to_string();
            Ok(Some(identity))
        });
        let result = get_service(store).get_identity_by_name("Minka").await;
        assert!(result.is_ok());
        assert_eq!(result.as_ref().unwrap().name, *"Minka");
    }

    #[tokio::test]
    async fn delete_identity_calls_store() {
        let mut store = MockContactStoreApi::new();
        store.expect_delete().returning(|_| Ok(()));
        let result = get_service(store)
            .delete_identity_by_name("some_name")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn update_identity_name_calls_store() {
        let mut store = MockContactStoreApi::new();
        store.expect_update_name().returning(|_, _| Ok(()));
        let result = get_service(store)
            .update_identity_name("old_name", "new_name")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn update_identity_calls_store() {
        let mut store = MockContactStoreApi::new();
        store.expect_update().returning(|_, _| Ok(()));
        let result = get_service(store)
            .update_identity("name", IdentityPublicData::new_empty())
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn is_known_npub_calls_store() {
        let mut store = MockContactStoreApi::new();
        store.expect_get_by_npub().returning(|_| {
            let mut identity = IdentityPublicData::new_empty();
            identity.name = "Minka".to_string();
            identity.node_id = "some_node_id".to_string();
            identity.nostr_npub = Some("some_npub".to_string());
            Ok(Some(identity))
        });
        let result = get_service(store).is_known_npub("some_npub").await;
        assert!(result.is_ok());
        assert!(result.as_ref().unwrap());
    }
}
