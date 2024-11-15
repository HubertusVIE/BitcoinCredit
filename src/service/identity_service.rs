use super::Result;
use crate::{
    bill::identity::Identity, constants::USEDNET, dht::Client,
    persistence::identity::IdentityStoreApi, util,
};
use async_trait::async_trait;
use openssl::{pkey::Private, rsa::Rsa};
use std::sync::Arc;

#[async_trait]
pub trait IdentityServiceApi: Send + Sync {
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
