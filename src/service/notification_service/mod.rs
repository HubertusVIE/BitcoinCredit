use std::sync::Arc;

use crate::persistence::NostrEventOffsetStoreApi;
use crate::persistence::{self, identity::IdentityStoreApi};
use crate::util::{self};
use crate::{config::Config, service::bill_service::BitcreditBill};
use async_trait::async_trait;
use default_service::DefaultNotificationService;
use handler::{LoggingEventHandler, NotificationHandlerApi};
#[cfg(test)]
use mockall::automock;
use serde_json::Value;
use serde::{Serialize, Deserialize};
use thiserror::Error;

#[cfg(test)]
pub mod test_utils;

pub mod default_service;
mod email;
mod email_lettre;
mod email_sendgrid;
mod event;
mod handler;
mod nostr;
mod transport;

pub use email::NotificationEmailTransportApi;
pub use event::{Event, EventEnvelope, EventType};
pub use nostr::{NostrClient, NostrConfig, NostrConsumer};
pub use transport::NotificationJsonTransportApi;

use super::contact_service::ContactServiceApi;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    /// json errors when serializing/deserializing notification events
    #[error("json serialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// errors stemming from lettre smtp transport
    #[error("lettre smtp transport error: {0}")]
    SmtpTransport(#[from] lettre::transport::smtp::Error),

    /// errors stemming from lettre stub transport (this will only be used for testing)
    #[error("lettre stub transport error: {0}")]
    StubTransport(#[from] lettre::transport::stub::Error),

    /// errors stemming from lettre email contents creation
    #[error("lettre email error: {0}")]
    LettreEmail(#[from] lettre::error::Error),

    /// errors stemming from lettre address parsing
    #[error("lettre address error: {0}")]
    LettreAddress(#[from] lettre::address::AddressError),

    /// some transports require a http client where we use reqwest
    #[error("http client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    #[error("nostr key error: {0}")]
    NostrKey(#[from] nostr_sdk::key::Error),

    #[error("nostr client error: {0}")]
    NostrClient(#[from] nostr_sdk::client::Error),

    #[error("crypto util error: {0}")]
    CryptoUtil(#[from] util::crypto::Error),

    #[error("Persistence error: {0}")]
    Persistence(#[from] persistence::Error),
}

/// Creates a new nostr client configured with the current identity user.
pub async fn create_nostr_client(
    config: &Config,
    identity_store: Arc<dyn IdentityStoreApi>,
) -> Result<NostrClient> {
    let keys = identity_store.get_or_create_key_pair().await?;

    let nostr_name = match identity_store.get().await {
        Ok(identity) => identity.get_nostr_name(),
        _ => "New user".to_owned(),
    };
    let config = NostrConfig::new(keys, vec![config.nostr_relay.clone()], nostr_name);
    NostrClient::new(&config).await
}

/// Creates a new notification service that will send events via the given Nostr json transport.
pub async fn create_notification_service(
    client: NostrClient,
) -> Result<Arc<dyn NotificationServiceApi>> {
    Ok(Arc::new(DefaultNotificationService::new(Box::new(client))))
}

/// Creates a new nostr consumer that will listen for incoming events and handle them
/// with the given handlers. The consumer is just set up here and needs to be started
/// via the run method later.
pub async fn create_nostr_consumer(
    client: NostrClient,
    contact_service: Arc<dyn ContactServiceApi>,
    nostr_event_offset_store: Arc<dyn NostrEventOffsetStoreApi>,
) -> Result<NostrConsumer> {
    // register the logging event handler for all events for now. Later we will probably
    // setup the handlers outside and pass them to the consumer via this functions arguments.
    let handlers: Vec<Box<dyn NotificationHandlerApi>> = vec![Box::new(LoggingEventHandler {
        event_types: EventType::all(),
    })];
    let consumer = NostrConsumer::new(client, contact_service, handlers, nostr_event_offset_store);
    Ok(consumer)
}

/// Send events via all channels required for the event type.
#[allow(dead_code)]
#[cfg_attr(test, automock)]
#[async_trait]
pub trait NotificationServiceApi: Send + Sync {
    /// Sent when: A bill is signed by: Drawer
    /// Receiver: Payer, Action: ApproveBill
    /// Receiver: Payee, Action: CheckBill
    async fn send_bill_is_signed_event(&self, bill: &BitcreditBill) -> Result<()>;

    /// Sent when: A bill is accepted by: Payer
    /// Receiver: Holder, Action: CheckBill
    async fn send_bill_is_accepted_event(&self, bill: &BitcreditBill) -> Result<()>;

    /// Sent when: A bill is requested to be accepted, Sent by: Holder
    /// Receiver: Payer, Action: ApproveBill
    async fn send_request_to_accept_event(&self, bill: &BitcreditBill) -> Result<()>;

    /// Sent when: A bill is requested to be paid, Sent by: Holder
    /// Receiver: Payer, Action: PayBill
    async fn send_request_to_pay_event(&self, bill: &BitcreditBill) -> Result<()>;

    /// Sent when: A bill is paid by: Payer (Bitcoin API)
    /// Receiver: Payee, Action: CheckBill
    async fn send_bill_is_paid_event(&self, bill: &BitcreditBill) -> Result<()>;

    /// Sent when: A bill is endorsed by: Previous Holder
    /// Receiver: NewHolder, Action: CheckBill
    async fn send_bill_is_endorsed_event(&self, bill: &BitcreditBill) -> Result<()>;

    /// Sent when: A bill is requested to be sold, Sent by: Holder
    /// Receiver: Buyer, Action: CheckBill (with buy page)
    async fn send_request_to_sell_event(&self, bill: &BitcreditBill) -> Result<()>;

    /// Sent when: A bill is sold by: Buyer (new holder)
    /// Receiver: Seller (old holder), Action: CheckBill (with pr key to take money)
    async fn send_bill_is_sold_event(&self, bill: &BitcreditBill) -> Result<()>;

    /// Sent when: A bill is requested to be minted, Sent by: Holder
    /// Receiver: Mint, Action: CheckBill (with generate quote page)
    async fn send_request_to_mint_event(&self, bill: &BitcreditBill) -> Result<()>;

    /// Sent when: A new quote is created, Sent by: Mint
    /// Receiver: Holder, Action: Check quote page
    async fn send_new_quote_event(&self, quote: &BitcreditBill) -> Result<()>;

    /// Sent when: A quote is approved by: Previous Holder
    /// Receiver: Mint (new holder), Action: CheckBill
    async fn send_quote_is_approved_event(&self, quote: &BitcreditBill) -> Result<()>;
}

/// A notification as it will be delivered to the UI.
///
/// A generic notification. Payload is unstructured json. The timestamp refers to the
/// time when the client received the notification. The type determines the payload
/// type and the reference_id is used to identify and optional other entity like a
/// Bill or Company.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    /// The unique id of the notification
    id: String,
    /// The type/topic of the notification
    notification_type: NotificationType,
    /// An optional reference to some other entity
    reference_id: Option<String>,
    /// A descriotion to quickly show to a user in the ui (probably a translation key)
    description: String,
    /// The timestamp when the notification was created
    timestamp: u64,
    /// Whether the notification is active or not. If active the user shold still perform
    /// some action to dismiss the notification.
    active: bool,
    /// Additional data to be used for notification specific logic
    payload: Option<Value>,
}

/// The type/topic of a notification we show to the user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationType {
    BillAction,
}
