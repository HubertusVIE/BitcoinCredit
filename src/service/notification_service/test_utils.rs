use crate::{service::bill_service::BitcreditBill, service::contact_service::IdentityPublicData};
use nostr_relay_builder::prelude::*;

use super::{
    email::EmailMessage, handler::NotificationHandlerApi, Event, EventEnvelope, EventType, Result,
};
use serde::{de::DeserializeOwned, Serialize};

/// These mocks might be useful for testing in other modules as well
use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::Mutex;

#[allow(dead_code)]
pub const NOSTR_KEY1: &str = "nsec1gr9hfpprzn0hs5xymm0h547f6nt9x2270cy9chyzq3leprnzr2csprwlds";
#[allow(dead_code)]
pub const NOSTR_KEY2: &str = "nsec1aqz0hckc4wmrzzucqp4cx89528qu6g8deez9m32p2x7ka5c6et8svxt0q3";
#[allow(dead_code)]
pub const NOSTR_NPUB1: &str = "npub1c504lwrnmrt7atmnxxlf54rw3pxjhjv3455h3flnham3hsgjcs0qjk962x";
#[allow(dead_code)]
pub const NOSTR_NPUB2: &str = "npub1zax8v4hasewaxducdn89clqwmv4dp84r6vgpls5j5xg6f7xda3fqh2sg75";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TestEventPayload {
    pub foo: String,
    pub bar: u32,
}

pub struct TestEventHandler<T: Serialize + DeserializeOwned> {
    pub called: Mutex<bool>,
    pub received_event: Mutex<Option<Event<T>>>,
    pub accepted_event: Option<EventType>,
}

impl<T: Serialize + DeserializeOwned> TestEventHandler<T> {
    pub fn new(accepted_event: Option<EventType>) -> Self {
        Self {
            called: Mutex::new(false),
            received_event: Mutex::new(None),
            accepted_event,
        }
    }
}

#[async_trait]
impl NotificationHandlerApi for TestEventHandler<TestEventPayload> {
    fn handles_event(&self, event_type: &EventType) -> bool {
        match &self.accepted_event {
            Some(e) => e == event_type,
            None => true,
        }
    }

    async fn handle_event(&self, event: EventEnvelope) -> Result<()> {
        *self.called.lock().await = true;
        let event: Event<TestEventPayload> = event.try_into()?;
        *self.received_event.lock().await = Some(event);
        Ok(())
    }
}

pub fn create_test_event_payload() -> TestEventPayload {
    TestEventPayload {
        foo: "foo".to_string(),
        bar: 42,
    }
}

pub fn create_test_event(event_type: &EventType) -> Event<TestEventPayload> {
    Event::new(
        event_type,
        "peer_id".to_string(),
        create_test_event_payload(),
    )
}

pub fn get_test_email_message() -> EmailMessage {
    EmailMessage {
        from: "sender@example.com".to_string(),
        to: "recipient@example.com".to_string(),
        subject: "Hello World".to_string(),
        body: "This is a test email.".to_string(),
    }
}

pub fn get_identity_public_data(
    peer_id: &str,
    email: &str,
    npub: Option<&str>,
    nostr_relay: Option<&str>,
) -> IdentityPublicData {
    let mut identity = IdentityPublicData::new_only_peer_id(peer_id.to_owned());
    identity.email = email.to_owned();
    identity.nostr_npub = npub.map(|npub| npub.to_owned());
    identity.nostr_relay = nostr_relay.map(|nostr_relay| nostr_relay.to_owned());
    identity
}

pub fn get_test_bitcredit_bill(
    name: &str,
    payer: &IdentityPublicData,
    payee: &IdentityPublicData,
) -> BitcreditBill {
    let mut bill = BitcreditBill::new_empty();
    bill.name = name.to_owned();
    bill.payee = payee.clone();
    bill.drawee = payer.clone();
    bill
}
pub async fn get_mock_relay() -> MockRelay {
    MockRelay::run().await.expect("could not create mock relay")
}
