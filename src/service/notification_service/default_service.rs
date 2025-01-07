use async_trait::async_trait;

use super::event::{ActionType, BillActionEventPayload, Event, EventType};
use super::transport::NotificationJsonTransportApi;
use super::{NotificationServiceApi, Result};
use crate::service::bill_service::BitcreditBill;

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

    use std::sync::Arc;

    use crate::persistence::nostr::MockNostrEventOffsetStoreApi;
    use crate::service::contact_service::MockContactServiceApi;
    use crate::service::notification_service::create_nostr_consumer;
    use crate::service::notification_service::transport::MockNotificationJsonTransportApi;

    use super::super::test_utils::{
        get_identity_public_data, get_mock_nostr_client, get_test_bitcredit_bill,
    };
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
