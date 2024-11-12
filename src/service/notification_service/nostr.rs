use async_trait::async_trait;
use log::error;
use nostr_sdk::prelude::*;
use std::str::FromStr;
use std::time::Duration;

use super::super::contact_service::IdentityPublicData;
use super::{EventEnvelope, NotificationJsonTransportApi, Result};

#[derive(Clone, Debug)]
pub struct NostrConfig {
    nsec: String,
    relays: Vec<String>,
    name: String,
    timeout: Option<Duration>,
}

/// A wrapper around nostr_sdk that implements the NotificationJsonTransportApi.
///
/// # Example:
/// ```
/// let config = NostrConfig {
///     nsec: "nsec1...".to_string(),
///     relays: vec!["wss://relay.example.com".to_string()],
///     name: "My Company".to_string(),
///     timeout: Some(Duration::from_secs(10)),
/// };
/// let transport = NostrClient::new(&config).await.unwrap();
/// transport.send(&recipient, event).await.unwrap();
/// ```
/// We use the latest GiftWrap and PrivateDirectMessage already with this if I
/// understand the nostr-sdk docs and sources correctly.
/// @see https://nips.nostr.com/59 and https://nips.nostr.com/17
#[derive(Clone, Debug)]
pub struct NostrClient {
    client: Client,
}

impl NostrClient {
    #[allow(dead_code)]
    pub async fn new(config: &NostrConfig) -> Result<Self> {
        let keys = Keys::parse(&config.nsec)?;
        let options = Options::new().connection_timeout(config.timeout);
        let client = Client::builder().signer(keys.clone()).opts(options).build();
        for relay in &config.relays {
            client.add_relay(relay).await?;
        }
        client.connect().await;
        let metadata = Metadata::new()
            .name(&config.name)
            .display_name(&config.name);
        client.set_metadata(&metadata).await?;
        Ok(Self { client })
    }
}

#[async_trait]
impl NotificationJsonTransportApi for NostrClient {
    async fn send(&self, recipient: &IdentityPublicData, event: EventEnvelope) -> Result<()> {
        if let Some(npub) = &recipient.nostr_npub {
            let public_key = PublicKey::from_str(npub)?;
            let message = serde_json::to_string(&event)?;
            self.client
                .send_private_msg(public_key, message, None)
                .await?;
        } else {
            error!("Nostr npub not found");
        }
        Ok(())
    }
}
