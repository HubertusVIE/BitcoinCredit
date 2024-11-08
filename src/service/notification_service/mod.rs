use crate::bill::BitcreditBill;
use async_trait::async_trait;

pub mod event;
pub mod handler;
pub mod transport;

#[cfg(test)]
pub mod test_utils;

pub use super::{Error, Result};
pub use event::{ActionType, BillActionEventPayload, Event, EventEnvelope, EventType};
pub use transport::NotificationJsonTransportApi;

/// Send events via all channels required for the event type.
#[allow(dead_code)]
#[async_trait]
pub trait NotificationServiceApi: Send + Sync {
    async fn send_bill_is_signed_event(&self, bill: &BitcreditBill) -> Result<()>;
}

pub struct DefaultNotificationService {
    notification_transport: Box<dyn NotificationJsonTransportApi>,
    email_transport: Box<dyn NotificationJsonTransportApi>,
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

        // TODO: This is just for demo purpose. The email transport will need
        // different payloads (rendered as html) and different recipients.
        self.email_transport.send(drawer_event.try_into()?).await?;
        Ok(())
    }
}
