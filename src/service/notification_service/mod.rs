use crate::bill::BitcreditBill;
use async_trait::async_trait;
use thiserror::Error;

#[cfg(test)]
pub mod test_utils;

pub mod email;
pub mod email_lettre;
pub mod email_sendgrid;
pub mod event;
pub mod handler;
pub mod transport;

pub use email::{EmailMessage, NotificationEmailTransportApi};
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
#[async_trait]
pub trait NotificationServiceApi: Send + Sync {
    async fn send_bill_is_signed_event(&self, bill: &BitcreditBill) -> Result<()>;
}

pub struct DefaultNotificationService {
    notification_transport: Box<dyn NotificationJsonTransportApi>,
    email_transport: Box<dyn NotificationEmailTransportApi>,
}

#[async_trait]
impl NotificationServiceApi for DefaultNotificationService {
    async fn send_bill_is_signed_event(&self, bill: &BitcreditBill) -> Result<()> {
        let event_type = EventType::BillSigned;

        let drawer_event = Event::new(
            &event_type,
            bill.drawer.peer_id.clone(),
            BillActionEventPayload {
                bill_name: bill.name.clone(),
                action_type: ActionType::CheckBill,
            },
        );
        let drawee_event = Event::new(
            &event_type,
            bill.drawee.peer_id.clone(),
            BillActionEventPayload {
                bill_name: bill.name.clone(),
                action_type: ActionType::ApproveBill,
            },
        );

        // TODO: This is just for demo purpose.
        self.notification_transport
            .send(drawer_event.clone().try_into()?)
            .await?;
        self.notification_transport
            .send(drawee_event.try_into()?)
            .await?;

        // TODO: This is just for demo purpose.
        if !bill.drawee.email.is_empty() && !bill.drawer.email.is_empty() {
            let email_message = EmailMessage {
                from: bill.drawer.email.to_owned(),
                to: bill.drawee.email.to_owned(),
                subject: "You have been billed".to_string(),
                body: "A bill has been signed and your approval is required.".to_string(),
            };
            self.email_transport.send(email_message).await?;
        }

        Ok(())
    }
}
