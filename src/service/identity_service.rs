use super::Result;
use crate::{constants::USEDNET, dht::Client, persistence::identity::IdentityStoreApi, util};
use async_trait::async_trait;
use borsh_derive::{BorshDeserialize, BorshSerialize};
use libp2p::identity::Keypair;
use libp2p::PeerId;
use openssl::{pkey::Private, rsa::Rsa};
use rocket::serde::{Deserialize, Serialize};
use rocket::FromForm;
use std::sync::Arc;

#[async_trait]
pub trait IdentityServiceApi: Send + Sync {
    /// Updates the identity
    async fn update_identity(&self, identity: &Identity) -> Result<()>;
    /// Gets the full local identity, including the key pair and peer id
    async fn get_full_identity(&self) -> Result<IdentityWithAll>;
    /// Gets the local identity
    async fn get_identity(&self) -> Result<Identity>;
    /// Gets the local peer_id
    async fn get_peer_id(&self) -> Result<PeerId>;
    /// Checks if the identity has been created
    async fn identity_exists(&self) -> bool;
    /// Creates the identity and returns it with it's key pair and peer id
    async fn create_identity(
        &self,
        name: String,
        company: String,
        date_of_birth: String,
        city_of_birth: String,
        country_of_birth: String,
        email: String,
        postal_address: String,
    ) -> Result<()>;
}

/// The identity service is responsible for managing the local identity and syncing it
/// with the dht data.
#[derive(Clone)]
pub struct IdentityService {
    client: Client,
    store: Arc<dyn IdentityStoreApi>,
}

impl IdentityService {
    pub fn new(client: Client, store: Arc<dyn IdentityStoreApi>) -> Self {
        Self { client, store }
    }
}

#[async_trait]
impl IdentityServiceApi for IdentityService {
    async fn get_full_identity(&self) -> Result<IdentityWithAll> {
        let identity = self.store.get_full().await?;
        Ok(identity)
    }

    async fn update_identity(&self, identity: &Identity) -> Result<()> {
        self.store.save(identity).await?;
        self.client
            .clone()
            .put_identity_public_data_in_dht()
            .await?;
        Ok(())
    }

    async fn get_identity(&self) -> Result<Identity> {
        let identity = self.store.get().await?;
        Ok(identity)
    }

    async fn get_peer_id(&self) -> Result<PeerId> {
        let peer_id = self.store.get_peer_id().await?;
        Ok(peer_id)
    }

    async fn identity_exists(&self) -> bool {
        self.store.exists().await
    }

    async fn create_identity(
        &self,
        name: String,
        company: String,
        date_of_birth: String,
        city_of_birth: String,
        country_of_birth: String,
        email: String,
        postal_address: String,
    ) -> Result<()> {
        let rsa: Rsa<Private> = util::rsa::generation_rsa_key();
        let private_key_pem: String = util::rsa::pem_private_key_from_rsa(&rsa);
        let public_key_pem: String = util::rsa::pem_public_key_from_rsa(&rsa);

        let s = bitcoin::secp256k1::Secp256k1::new();
        let private_key = bitcoin::PrivateKey::new(
            s.generate_keypair(&mut bitcoin::secp256k1::rand::thread_rng())
                .0,
            USEDNET,
        );
        let public_key = private_key.public_key(&s).to_string();
        let private_key = private_key.to_string();

        let identity = Identity {
            name,
            company,
            date_of_birth,
            city_of_birth,
            country_of_birth,
            email,
            postal_address,
            public_key_pem,
            private_key_pem,
            bitcoin_public_key: public_key,
            bitcoin_private_key: private_key.clone(),
            nostr_npub: None,
        };
        self.store.save(&identity).await?;
        self.client
            .clone()
            .put_identity_public_data_in_dht()
            .await?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct IdentityWithAll {
    pub identity: Identity,
    pub peer_id: PeerId,
    #[allow(dead_code)]
    pub key_pair: Keypair,
}

#[derive(BorshSerialize, BorshDeserialize, FromForm, Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct Identity {
    pub name: String,
    pub company: String,
    pub date_of_birth: String,
    pub city_of_birth: String,
    pub country_of_birth: String,
    pub email: String,
    pub postal_address: String,
    pub public_key_pem: String,
    pub private_key_pem: String,
    pub bitcoin_public_key: String,
    pub bitcoin_private_key: String,
    pub nostr_npub: Option<String>,
}

macro_rules! update_field {
    ($self:expr, $other:expr, $field:ident) => {
        if !$other.$field.is_empty() {
            $self.$field = $other.$field.clone();
        }
    };
}

impl Identity {
    pub fn new_empty() -> Self {
        Self {
            name: "".to_string(),
            company: "".to_string(),
            date_of_birth: "".to_string(),
            city_of_birth: "".to_string(),
            bitcoin_public_key: "".to_string(),
            postal_address: "".to_string(),
            public_key_pem: "".to_string(),
            email: "".to_string(),
            country_of_birth: "".to_string(),
            private_key_pem: "".to_string(),
            bitcoin_private_key: "".to_string(),
            nostr_npub: None,
        }
    }

    fn all_changeable_fields_empty(&self) -> bool {
        self.name.is_empty()
            && self.company.is_empty()
            && self.postal_address.is_empty()
            && self.email.is_empty()
    }

    fn all_changeable_fields_equal_to(&self, other: &Self) -> bool {
        self.name == other.name
            && self.company == other.company
            && self.postal_address == other.postal_address
            && self.email == other.email
    }

    pub fn update_valid(&self, other: &Self) -> bool {
        if other.all_changeable_fields_empty() {
            return false;
        }
        if self.all_changeable_fields_equal_to(other) {
            return false;
        }
        true
    }

    pub fn update_from(&mut self, other: &Identity) {
        update_field!(self, other, name);
        update_field!(self, other, company);
        update_field!(self, other, postal_address);
        update_field!(self, other, email);
    }
}
