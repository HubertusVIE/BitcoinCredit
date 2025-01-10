use super::{NotificationType, Result};
use std::sync::Arc;

use crate::{
    persistence::notification::NotificationStoreApi,
    service::notification_service::event::{BillActionEventPayload, Event},
};

use super::{
    handler::NotificationHandlerApi, push_notification::PushApi, EventEnvelope, EventType,
    Notification,
};
use async_trait::async_trait;

#[derive(Clone)]
pub struct BillActionEventHandler {
    notification_store: Arc<dyn NotificationStoreApi>,
    push_service: Arc<dyn PushApi>,
}

impl BillActionEventHandler {
    pub fn new(
        notification_store: Arc<dyn NotificationStoreApi>,
        push_service: Arc<dyn PushApi>,
    ) -> Self {
        Self {
            notification_store,
            push_service,
        }
    }

    fn event_description(&self, event_type: &EventType) -> String {
        match event_type {
            EventType::BillSigned => "Bill has been signed".to_string(),
            EventType::BillAccepted => "Bill has been accepted".to_string(),
            EventType::BillAcceptanceRequested => "Bill should be accepted".to_string(),
            EventType::BillPaymentRequested => "Bill should be paid".to_string(),
            EventType::BillSellRequested => "Bill should be sold".to_string(),
            EventType::BillPaid => "Bill has been paid".to_string(),
            EventType::BillEndorsed => "Bill has been endorsed".to_string(),
            EventType::BillSold => "Bill has been sold".to_string(),
            EventType::BillMintingRequested => "Bill should be minted".to_string(),
            EventType::BillNewQuote => "New quote has been added".to_string(),
            EventType::BillQuoteApproved => "Quote has been approved".to_string(),
        }
    }
}

#[async_trait]
impl NotificationHandlerApi for BillActionEventHandler {
    fn handles_event(&self, _event_type: &EventType) -> bool {
        true
    }

    async fn handle_event(&self, event: EventEnvelope) -> Result<()> {
        let event: Option<Event<BillActionEventPayload>> = event.try_into().ok();
        if let Some(event) = event {
            // create notification
            let notification = Notification::new_bill_notification(
                &event.data.bill_id,
                &self.event_description(&event.event_type),
                Some(serde_json::to_value(&event.data)?),
            );

            // mark current active Bill event as done if any
            if let Some(currently_active) = self
                .notification_store
                .get_latest_by_reference(&event.data.bill_id, NotificationType::Bill)
                .await?
            {
                self.notification_store
                    .mark_as_done(&currently_active.id)
                    .await?;
            }

            // save new notification to database
            self.notification_store.add(notification.clone()).await?;

            // send push notification to connected clients
            self.push_service
                .send(serde_json::to_value(notification)?)
                .await;
        }
        Ok(())
    }
}
