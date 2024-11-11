use crate::bill::BitcreditBill;
use async_trait::async_trait;
#[cfg(test)]
use mockall::automock;
use thiserror::Error;

#[cfg(test)]
pub mod test_utils;

pub mod email;
pub mod email_lettre;
pub mod email_sendgrid;
pub mod event;
pub mod handler;
pub mod transport;

pub use email::NotificationEmailTransportApi;
pub use event::{ActionType, BillActionEventPayload, Event, EventEnvelope, EventType};
pub use transport::NotificationJsonTransportApi;

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
}

/// Send events via all channels required for the event type.
#[allow(dead_code)]
#[cfg_attr(test, automock)]
#[async_trait]
pub trait NotificationServiceApi: Send + Sync {
    async fn send_bill_is_signed_event(&self, bill: &BitcreditBill) -> Result<()>;
}

/// A default implementation of the NotificationServiceApi that can
/// send events via json and email transports.
#[allow(dead_code)]
pub struct DefaultNotificationService {
    notification_transport: Box<dyn NotificationJsonTransportApi>,
}

#[async_trait]
impl NotificationServiceApi for DefaultNotificationService {
    async fn send_bill_is_signed_event(&self, bill: &BitcreditBill) -> Result<()> {
        let event_type = EventType::BillSigned;

        let payer_event = Event::new(
            &event_type,
            bill.drawee.peer_id.clone(),
            BillActionEventPayload {
                bill_name: bill.name.clone(),
                action_type: ActionType::ApproveBill,
            },
        );
        let payee_event = Event::new(
            &event_type,
            bill.payee.peer_id.clone(),
            BillActionEventPayload {
                bill_name: bill.name.clone(),
                action_type: ActionType::CheckBill,
            },
        );

        self.notification_transport
            .send(payer_event.clone().try_into()?)
            .await?;

        self.notification_transport
            .send(payee_event.try_into()?)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::test_utils::{get_identity_public_data, get_test_bitcredit_bill};
    use super::transport::MockNotificationJsonTransportApi;
    use super::*;

    #[tokio::test]
    async fn test_send_bill_is_signed_event() {
        // given a payer and payee with a new bill
        let payer = get_identity_public_data("payer", "payer@example.com");
        let payee = get_identity_public_data("payee", "payee@example.com");
        let bill = get_test_bitcredit_bill("bill", &payer, &payee);

        let mut mock = MockNotificationJsonTransportApi::new();
        mock.expect_send()
            .withf(|e| e.peer_id == "payer" && e.event_type == EventType::BillSigned)
            .returning(|_| Ok(()));

        mock.expect_send()
            .withf(|e| e.peer_id == "payee" && e.event_type == EventType::BillSigned)
            .returning(|_| Ok(()));

        let service = DefaultNotificationService {
            notification_transport: Box::new(mock),
        };

        service
            .send_bill_is_signed_event(&bill)
            .await
            .expect("failed to send event");
    }
}
