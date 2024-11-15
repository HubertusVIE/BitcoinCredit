use super::Result;
use crate::{
    bill::identity::{Identity, IdentityWithAll},
    constants::USEDNET,
    dht::Client,
    persistence::identity::IdentityStoreApi,
    util,
};
use async_trait::async_trait;
use libp2p::PeerId;
use openssl::{pkey::Private, rsa::Rsa};
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
