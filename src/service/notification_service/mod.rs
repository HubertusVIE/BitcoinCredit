use std::sync::Arc;

use crate::persistence::NostrEventOffsetStoreApi;
use crate::persistence::{self, identity::IdentityStoreApi};
use crate::util::{self};
use crate::{config::Config, service::bill_service::BitcreditBill};
use async_trait::async_trait;
use handler::{LoggingEventHandler, NotificationHandlerApi};
#[cfg(test)]
use mockall::automock;
use thiserror::Error;

#[cfg(test)]
pub mod test_utils;

mod email;
mod email_lettre;
mod email_sendgrid;
mod event;
mod handler;
mod nostr;
mod transport;

pub use email::NotificationEmailTransportApi;
pub use event::{ActionType, BillActionEventPayload, Event, EventEnvelope, EventType};
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

/// A default implementation of the NotificationServiceApi that can
/// send events via json and email transports.
#[allow(dead_code)]
pub struct DefaultNotificationService {
    notification_transport: Box<dyn NotificationJsonTransportApi>,
}

impl DefaultNotificationService {
    pub fn new(notification_transport: Box<dyn NotificationJsonTransportApi>) -> Self {
        Self {
            notification_transport,
        }
    }
}

#[async_trait]
impl NotificationServiceApi for DefaultNotificationService {
    async fn send_bill_is_signed_event(&self, bill: &BitcreditBill) -> Result<()> {
        let event_type = EventType::BillSigned;

        let payer_event = Event::new(
            event_type.to_owned(),
            &bill.drawee.node_id,
            BillActionEventPayload {
                bill_id: bill.name.clone(),
                action_type: ActionType::ApproveBill,
            },
        );
        let payee_event = Event::new(
            event_type,
            &bill.payee.node_id,
            BillActionEventPayload {
                bill_id: bill.name.clone(),
                action_type: ActionType::CheckBill,
            },
        );

        self.notification_transport
            .send(&bill.drawee, payer_event.try_into()?)
            .await?;

        self.notification_transport
            .send(&bill.payee, payee_event.try_into()?)
            .await?;

        Ok(())
    }

    async fn send_bill_is_accepted_event(&self, bill: &BitcreditBill) -> Result<()> {
        let event = Event::new(
            EventType::BillAccepted,
            &bill.payee.node_id,
            BillActionEventPayload {
                bill_id: bill.name.clone(),
                action_type: ActionType::CheckBill,
            },
        );

        self.notification_transport
            .send(&bill.payee, event.try_into()?)
            .await?;
        Ok(())
    }

    async fn send_request_to_accept_event(&self, bill: &BitcreditBill) -> Result<()> {
        let event = Event::new(
            EventType::BillAcceptanceRequested,
            &bill.drawee.node_id,
            BillActionEventPayload {
                bill_id: bill.name.clone(),
                action_type: ActionType::ApproveBill,
            },
        );
        self.notification_transport
            .send(&bill.drawee, event.try_into()?)
            .await?;
        Ok(())
    }

    async fn send_request_to_pay_event(&self, bill: &BitcreditBill) -> Result<()> {
        let event = Event::new(
            EventType::BillPaymentRequested,
            &bill.drawee.node_id,
            BillActionEventPayload {
                bill_id: bill.name.clone(),
                action_type: ActionType::PayBill,
            },
        );
        self.notification_transport
            .send(&bill.drawee, event.try_into()?)
            .await?;
        Ok(())
    }

    async fn send_bill_is_paid_event(&self, bill: &BitcreditBill) -> Result<()> {
        let event = Event::new(
            EventType::BillPaid,
            &bill.payee.node_id,
            BillActionEventPayload {
                bill_id: bill.name.clone(),
                action_type: ActionType::CheckBill,
            },
        );

        self.notification_transport
            .send(&bill.payee, event.try_into()?)
            .await?;
        Ok(())
    }

    async fn send_bill_is_endorsed_event(&self, bill: &BitcreditBill) -> Result<()> {
        let event = Event::new(
            EventType::BillEndorsed,
            &bill.endorsee.node_id,
            BillActionEventPayload {
                bill_id: bill.name.clone(),
                action_type: ActionType::CheckBill,
            },
        );

        self.notification_transport
            .send(&bill.endorsee, event.try_into()?)
            .await?;
        Ok(())
    }

    async fn send_request_to_sell_event(&self, bill: &BitcreditBill) -> Result<()> {
        let event = Event::new(
            EventType::BillSellRequested,
            &bill.endorsee.node_id,
            BillActionEventPayload {
                bill_id: bill.name.clone(),
                action_type: ActionType::CheckBill,
            },
        );
        self.notification_transport
            .send(&bill.endorsee, event.try_into()?)
            .await?;
        Ok(())
    }

    async fn send_bill_is_sold_event(&self, bill: &BitcreditBill) -> Result<()> {
        let event = Event::new(
            EventType::BillSold,
            &bill.drawee.node_id,
            BillActionEventPayload {
                bill_id: bill.name.clone(),
                action_type: ActionType::CheckBill,
            },
        );
        self.notification_transport
            .send(&bill.drawee, event.try_into()?)
            .await?;
        Ok(())
    }

    async fn send_request_to_mint_event(&self, bill: &BitcreditBill) -> Result<()> {
        let event = Event::new(
            EventType::BillMintingRequested,
            &bill.endorsee.node_id,
            BillActionEventPayload {
                bill_id: bill.name.clone(),
                action_type: ActionType::CheckBill,
            },
        );
        self.notification_transport
            .send(&bill.endorsee, event.try_into()?)
            .await?;
        Ok(())
    }

    async fn send_new_quote_event(&self, _bill: &BitcreditBill) -> Result<()> {
        // @TODO: How do we know the quoting participants
        Ok(())
    }

    async fn send_quote_is_approved_event(&self, _bill: &BitcreditBill) -> Result<()> {
        // @TODO: How do we address a mint ???
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use persistence::nostr::MockNostrEventOffsetStoreApi;
    use test_utils::get_mock_nostr_client;

    use crate::service::contact_service::MockContactServiceApi;

    use super::test_utils::{get_identity_public_data, get_test_bitcredit_bill};
    use super::transport::MockNotificationJsonTransportApi;
    use super::*;

    #[tokio::test]
    async fn test_send_bill_is_signed_event() {
        // given a payer and payee with a new bill
        let payer = get_identity_public_data("drawee", "drawee@example.com", None, None);
        let payee = get_identity_public_data("payee", "payee@example.com", None, None);
        let bill = get_test_bitcredit_bill("bill", &payer, &payee, None, None);

        let mut mock = MockNotificationJsonTransportApi::new();
        mock.expect_send()
            .withf(|r, e| {
                let valid_node_id = r.node_id == "drawee" && e.node_id == "drawee";
                let valid_event_type = e.event_type == EventType::BillSigned;
                let event: Event<BillActionEventPayload> = e.clone().try_into().unwrap();
                valid_node_id
                    && valid_event_type
                    && event.data.action_type == ActionType::ApproveBill
            })
            .returning(|_, _| Ok(()));

        mock.expect_send()
            .withf(|r, e| {
                let valid_node_id = r.node_id == "payee" && e.node_id == "payee";
                let valid_event_type = e.event_type == EventType::BillSigned;
                let event: Event<BillActionEventPayload> = e.clone().try_into().unwrap();
                valid_node_id && valid_event_type && event.data.action_type == ActionType::CheckBill
            })
            .returning(|_, _| Ok(()));

        let service = DefaultNotificationService {
            notification_transport: Box::new(mock),
        };

        service
            .send_bill_is_signed_event(&bill)
            .await
            .expect("failed to send event");
    }

    #[tokio::test]
    async fn test_send_bill_is_accepted_event() {
        let bill = get_test_bill();

        // should send accepted to payee
        let service =
            setup_service_expectation("payee", EventType::BillAccepted, ActionType::CheckBill);

        service
            .send_bill_is_accepted_event(&bill)
            .await
            .expect("failed to send event");
    }

    #[tokio::test]
    async fn test_send_request_to_accept_event() {
        let bill = get_test_bill();

        // should send request to accept to drawee
        let service = setup_service_expectation(
            "drawee",
            EventType::BillAcceptanceRequested,
            ActionType::ApproveBill,
        );

        service
            .send_request_to_accept_event(&bill)
            .await
            .expect("failed to send event");
    }

    #[tokio::test]
    async fn test_send_request_to_pay_event() {
        let bill = get_test_bill();

        // should send request to pay to drawee
        let service = setup_service_expectation(
            "drawee",
            EventType::BillPaymentRequested,
            ActionType::PayBill,
        );

        service
            .send_request_to_pay_event(&bill)
            .await
            .expect("failed to send event");
    }

    #[tokio::test]
    async fn test_send_bill_is_paid_event() {
        let bill = get_test_bill();

        // should send paid to payee
        let service =
            setup_service_expectation("payee", EventType::BillPaid, ActionType::CheckBill);

        service
            .send_bill_is_paid_event(&bill)
            .await
            .expect("failed to send event");
    }

    #[tokio::test]
    async fn test_send_bill_is_endorsed_event() {
        let bill = get_test_bill();

        // should send endorsed to endorsee
        let service =
            setup_service_expectation("endorsee", EventType::BillEndorsed, ActionType::CheckBill);

        service
            .send_bill_is_endorsed_event(&bill)
            .await
            .expect("failed to send event");
    }

    #[tokio::test]
    async fn test_send_request_to_sell_event() {
        let bill = get_test_bill();

        // should send request to sell to endorsee
        let service = setup_service_expectation(
            "endorsee",
            EventType::BillSellRequested,
            ActionType::CheckBill,
        );

        service
            .send_request_to_sell_event(&bill)
            .await
            .expect("failed to send event");
    }

    #[tokio::test]
    async fn test_send_bill_is_sold_event() {
        let bill = get_test_bill();

        // should send sold event to drawee
        let service =
            setup_service_expectation("drawee", EventType::BillSold, ActionType::CheckBill);

        service
            .send_bill_is_sold_event(&bill)
            .await
            .expect("failed to send event");
    }

    #[tokio::test]
    async fn test_send_request_to_mint_event() {
        let bill = get_test_bill();

        // should send minting requested to endorsee (mint)
        let service = setup_service_expectation(
            "endorsee",
            EventType::BillMintingRequested,
            ActionType::CheckBill,
        );

        service
            .send_request_to_mint_event(&bill)
            .await
            .expect("failed to send event");
    }

    fn setup_service_expectation(
        node_id: &str,
        event_type: EventType,
        action_type: ActionType,
    ) -> DefaultNotificationService {
        let node_id = node_id.to_owned();
        let mut mock = MockNotificationJsonTransportApi::new();
        mock.expect_send()
            .withf(move |r, e| {
                let valid_node_id = r.node_id == node_id && e.node_id == node_id;
                let valid_event_type = e.event_type == event_type;
                let event: Event<BillActionEventPayload> = e.clone().try_into().unwrap();
                valid_node_id && valid_event_type && event.data.action_type == action_type
            })
            .returning(|_, _| Ok(()));
        DefaultNotificationService {
            notification_transport: Box::new(mock),
        }
    }

    fn get_test_bill() -> BitcreditBill {
        get_test_bitcredit_bill(
            "bill",
            &get_identity_public_data("drawee", "drawee@example.com", None, None),
            &get_identity_public_data("payee", "payee@example.com", None, None),
            Some(&get_identity_public_data(
                "drawer",
                "drawer@example.com",
                None,
                None,
            )),
            Some(&get_identity_public_data(
                "endorsee",
                "endorsee@example.com",
                None,
                None,
            )),
        )
    }

    #[tokio::test]
    async fn test_create_nostr_consumer() {
        let client = get_mock_nostr_client().await;
        let contact_service = Arc::new(MockContactServiceApi::new());
        let store = Arc::new(MockNostrEventOffsetStoreApi::new());
        let _ = create_nostr_consumer(client, contact_service, store).await;
    }
}
