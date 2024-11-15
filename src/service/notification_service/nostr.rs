use async_trait::async_trait;
use log::{error, trace, warn};
use nostr_sdk::prelude::*;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::service::contact_service::ContactServiceApi;

use super::super::contact_service::IdentityPublicData;
use super::handler::NotificationHandlerApi;
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
    pub public_key: PublicKey,
    pub client: Client,
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
        Ok(Self {
            public_key: keys.public_key(),
            client,
        })
    }

    /// Subscribe to some nostr events with a filter
    pub async fn subscribe(&self, subscription: Filter) -> Result<()> {
        self.client.subscribe(vec![subscription], None).await?;
        Ok(())
    }

    /// Unwrap envelope from private direct message
    pub async fn unwrap_envelope(
        &self,
        note: RelayPoolNotification,
    ) -> Option<(EventEnvelope, PublicKey, EventId)> {
        let mut result: Option<(EventEnvelope, PublicKey, EventId)> = None;
        if let RelayPoolNotification::Event { event, .. } = note {
            if event.kind == Kind::GiftWrap {
                result = match self.client.unwrap_gift_wrap(&event).await {
                    Ok(UnwrappedGift { rumor, sender }) => {
                        extract_event_envelope(rumor).map(|e| (e, sender, event.id))
                    }
                    Err(e) => {
                        error!("Unwrapping gift wrap failed: {e}");
                        None
                    }
                }
            }
        }
        result
    }
}

#[async_trait]
impl NotificationJsonTransportApi for NostrClient {
    async fn send(&self, recipient: &IdentityPublicData, event: EventEnvelope) -> Result<()> {
        if let Some(npub) = &recipient.nostr_npub {
            let public_key = PublicKey::from_str(npub)?;
            let message = serde_json::to_string(&event)?;
            if let Some(relay) = &recipient.nostr_relay {
                self.client
                    .send_private_msg_to(vec![relay], public_key, message, None)
                    .await?;
            } else {
                self.client
                    .send_private_msg(public_key, message, None)
                    .await?;
            }
        } else {
            error!(
                "Try to send Nostr message but Nostr npub not found in contact {}",
                recipient.name
            );
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct NostrConsumer {
    client: NostrClient,
    event_handlers: Arc<Vec<Box<dyn NotificationHandlerApi>>>,
    contact_service: Arc<dyn ContactServiceApi>,
}

impl NostrConsumer {
    #[allow(dead_code)]
    pub fn new(
        client: NostrClient,
        contact_service: Arc<dyn ContactServiceApi>,
        event_handlers: Vec<Box<dyn NotificationHandlerApi>>,
    ) -> Self {
        Self {
            client,
            event_handlers: Arc::new(event_handlers),
            contact_service,
        }
    }

    #[allow(dead_code)]
    pub async fn start(&self) -> Result<JoinHandle<()>> {
        // move dependencies into thread scope
        let client = self.client.clone();
        let event_handlers = self.event_handlers.clone();
        let contact_service = self.contact_service.clone();

        // subscribe only to private messages sent to our pubkey
        client
            .subscribe(Filter::new().pubkey(client.public_key).kind(Kind::GiftWrap))
            .await
            .expect("Failed to subscribe to Nostr events");

        // run subscription in a tokio task
        let handle = tokio::spawn(async move {
            client
                .client
                .handle_notifications(|note| async {
                    if let Some((envelope, sender, _event_id)) = client.unwrap_envelope(note).await
                    {
                        // TODO: Check if we already processed this event via event_id
                        // We only want to handle events from known contacts
                        if let Ok(sender) = sender.to_bech32() {
                            trace!("Received event: {envelope:?} from {sender:?}");
                            if contact_service.is_known_npub(sender.as_str()).await? {
                                trace!("Received event: {envelope:?} from {sender:?}");
                                handle_event(envelope, &event_handlers).await?;
                            }
                        }
                    };
                    Ok(false)
                })
                .await
                .expect("Nostr notification handler failed");
        });
        Ok(handle)
    }
}

fn extract_event_envelope(rumor: UnsignedEvent) -> Option<EventEnvelope> {
    if rumor.kind == Kind::PrivateDirectMessage {
        match serde_json::from_str::<EventEnvelope>(rumor.content.as_str()) {
            Ok(envelope) => Some(envelope),
            Err(e) => {
                error!("Json deserializing event envelope failed: {e}");
                None
            }
        }
    } else {
        None
    }
}

/// Handle extracted event with given handlers.
async fn handle_event(
    event: EventEnvelope,
    handlers: &Arc<Vec<Box<dyn NotificationHandlerApi>>>,
) -> Result<()> {
    let event_type = &event.event_type;
    let mut times = 0;
    for handler in handlers.iter() {
        if handler.handles_event(event_type) {
            match handler.handle_event(event.to_owned()).await {
                Ok(_) => times += 1,
                Err(e) => error!("Nostr event handler failed: {e}"),
            }
        }
    }
    if times < 1 {
        warn!("No handler subscribed for event: {event:?}");
    } else {
        trace!("{event_type:?} event handled successfully {times} times");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use mockall::predicate;
    use tokio::time;

    use super::super::test_utils::{get_mock_relay, NOSTR_KEY1};
    use super::{NostrClient, NostrConfig, NostrConsumer};
    use crate::service::notification_service::Event;
    use crate::service::{
        contact_service::MockContactServiceApi,
        notification_service::{
            handler::MockNotificationHandlerApi, test_utils::*, EventType,
            NotificationJsonTransportApi,
        },
    };

    /// When testing with the mock relay we need to be careful. It is always
    /// listening on the same port and will not start multiple times. If we
    /// share the instance tests will fail with events from other tests.
    #[tokio::test]
    async fn test_send_and_receive_event() {
        let relay = get_mock_relay().await;
        let url = relay.url();

        // given two clients
        let config1 = NostrConfig {
            nsec: NOSTR_KEY1.to_string(),
            relays: vec![url.to_string()],
            name: "BcrDamus1".to_string(),
            timeout: Some(Duration::from_secs(10)),
        };
        let client1 = NostrClient::new(&config1)
            .await
            .expect("failed to create nostr client 1");

        let config2 = NostrConfig {
            nsec: NOSTR_KEY2.to_string(),
            relays: vec![url.to_string()],
            name: "BcrDamus2".to_string(),
            timeout: Some(Duration::from_secs(10)),
        };
        let client2 = NostrClient::new(&config2)
            .await
            .expect("failed to create nostr client 2");

        // and a contact we want to send an event to
        let contact =
            get_identity_public_data("payee", "payee@example.com", Some(NOSTR_NPUB2), Some(&url));
        let mut event = create_test_event(&EventType::BillSigned);
        event.peer_id = contact.peer_id.to_owned();

        // expect the receiver to check if the sender contact is known
        let mut contact_service = MockContactServiceApi::new();
        contact_service
            .expect_is_known_npub()
            .with(predicate::eq(NOSTR_NPUB1))
            .returning(|_| Ok(true));

        // expect a handler that is subscribed to the event type w sent
        let mut handler = MockNotificationHandlerApi::new();
        handler
            .expect_handles_event()
            .with(predicate::eq(&EventType::BillSigned))
            .returning(|_| true);

        // expect a handler receiving the event we sent
        let expected_event: Event<TestEventPayload> = event.clone();
        handler
            .expect_handle_event()
            .withf(move |e| {
                let expected = expected_event.clone();
                let received: Event<TestEventPayload> =
                    e.clone().try_into().expect("could not convert event");
                let valid_type = received.event_type == expected.event_type;
                let valid_receiver = received.peer_id == expected.peer_id;
                let valid_payload = received.data.foo == expected.data.foo;
                valid_type && valid_receiver && valid_payload
            })
            .returning(|_| Ok(()));

        // we start the consumer
        let consumer =
            NostrConsumer::new(client2, Arc::new(contact_service), vec![Box::new(handler)]);
        let handle = consumer
            .start()
            .await
            .expect("failed to start nostr consumer");

        // and send an event
        client1
            .send(&contact, event.try_into().expect("could not convert event"))
            .await
            .expect("failed to send event");

        // give it a little bit of time to process the event
        time::sleep(Duration::from_millis(100)).await;
        handle.abort();
    }
}
