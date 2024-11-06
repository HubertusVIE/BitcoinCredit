use crate::bill::BitcreditBill;

use super::Result;
use async_trait::async_trait;
use log::info;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

/// The different types of events that can be sent via this service.
/// For now we only have Bill events and this needs some clippy
/// exceptions here. As soon as we have other event topics, we can
/// add new types here and remove the clippy exceptions.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[allow(clippy::enum_variant_names, dead_code)]
pub enum EventType {
    BillSigned,
    BillAccepted,
    BillAcceptanceRequested,
    BillPaymentRequested,
    BillSellRequested,
    BillPaid,
    BillEndorsed,
    BillSold,
    BillMintingRequested,
    BillNewQuote,
    BillQuoteApproved,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[allow(clippy::enum_variant_names, dead_code)]
pub enum ActionType {
    ApproveBill,
    CheckBill,
}

/// Can be used for all events that are just signalling an action
/// to be performed by the receiver. If we want to also notify
/// recipients via email or push notifications, we probably need to
/// add more fields here and create multiple event types.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BillActionEventPayload {
    bill_name: String,
    action_type: ActionType,
}

/// A generic event that can be sent to a specific recipient
/// and is serializable. The recipient is currently just a string,
/// and we have to decide what the identifier is.
/// This event should contain all the information that is needed
/// to send to different channels including email, push and Nostr.
#[derive(Serialize, Debug, Clone)]
pub struct Event<T: Serialize> {
    pub event_type: EventType,
    pub version: String,
    pub peer_id: String,
    pub data: T,
}

impl<T: Serialize> Event<T> {
    #[allow(dead_code)]
    pub fn new(event_type: &EventType, peer_id: String, data: T) -> Self {
        Self {
            event_type: event_type.to_owned(),
            version: "1.0".to_string(),
            peer_id,
            data,
        }
    }
}

/// When we receive an event, we need to know what type it is and
/// how to handle it. This payload envelope allows us to find out
/// the type of event to later deserialize the data into the correct
/// type.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EventEnvelope {
    pub event_type: EventType,
    pub version: String,
    pub peer_id: String,
    pub data: Value,
}

impl<T: Serialize> TryFrom<Event<T>> for EventEnvelope {
    type Error = super::Error;

    fn try_from(event: Event<T>) -> Result<Self> {
        Ok(Self {
            event_type: event.event_type,
            version: event.version,
            peer_id: event.peer_id,
            data: serde_json::to_value(event.data)?,
        })
    }
}

/// Allows generic deserialization of an event from an envelope.
/// # Example
///
/// ```
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct MyEventPayload {
///     foo: String,
///     bar: u32,
/// }
///
/// let payload = MyEventPayload {
///     foo: "foo".to_string(),
///     bar: 42,
/// };
///
/// let event = Event::new(EventType::BillSigned, "recipient".to_string(), payload);
/// let event: EventEnvelope = event.try_into().unwrap();
/// let deserialized_event: Event<MyEventPayload> = event.try_into().unwrap();
/// assert_eq!(deserialized_event.data, payload);
///
/// ```
///
impl<T: DeserializeOwned + Serialize> TryFrom<EventEnvelope> for Event<T> {
    type Error = super::Error;
    fn try_from(envelope: EventEnvelope) -> Result<Self> {
        let data: T = serde_json::from_value(envelope.data)?;
        Ok(Self {
            event_type: envelope.event_type,
            version: envelope.version,
            peer_id: envelope.peer_id,
            data,
        })
    }
}

/// Send an event via all channels required for the event type.
#[allow(dead_code)]
#[async_trait]
pub trait NotificationServiceApi: Send + Sync {
    async fn send_bill_is_signed_event(&self, bill: &BitcreditBill) -> Result<()>;
}

pub struct DefaultNotificationService {
    notification_transport: Box<dyn NotificationTransportApi>,
    email_transport: Box<dyn NotificationTransportApi>,
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
                action_type: ActionType::CheckBill,
            },
        );

        self.notification_transport
            .send(drawer_event.clone().try_into()?)
            .await?;
        self.notification_transport
            .send(drawee_event.try_into()?)
            .await?;
        self.email_transport.send(drawer_event.try_into()?).await?;
        Ok(())
    }
}

#[async_trait]
pub trait NotificationTransportApi: Send + Sync {
    async fn send(&self, event: EventEnvelope) -> Result<()>;
}

/// Handle an event when we receive it from a channel.
#[allow(dead_code)]
#[async_trait]
pub trait NotificationHandlerApi: Send + Sync {
    /// Whether this handler handles the given event type.
    fn handles_event(&self, event_type: &EventType) -> bool;

    /// Handle the event. This is called by the notification processor which should
    /// have checked the event type before calling this method. The actual implementation
    /// should be able to deserialize the data into its T type because the EventType
    /// determines the T type.
    async fn handle_event(&self, event: EventEnvelope) -> Result<()>;
}

/// A dummy transport that logs all events that are sent.
pub struct LoggingNotificationTransport {
    name: String,
}

#[async_trait]
impl NotificationTransportApi for LoggingNotificationTransport {
    async fn send(&self, event: EventEnvelope) -> Result<()> {
        info!(
            "Sending {} event: {:?}({}) with payload: {:?} to peer: {}",
            self.name, event.event_type, event.version, event.data, event.peer_id
        );
        Ok(())
    }
}

/// Logs all events that are received and registered in the event_types.
pub struct LoggingEventHandler {
    event_types: Vec<EventType>,
}

/// Just a dummy handler that logs the event and returns Ok(())
#[async_trait]
impl NotificationHandlerApi for LoggingEventHandler {
    fn handles_event(&self, event_type: &EventType) -> bool {
        self.event_types.contains(event_type)
    }

    async fn handle_event(&self, event: EventEnvelope) -> Result<()> {
        info!("Received event: {event:?}");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use test_utils::*;

    use super::*;

    #[test]
    fn test_event_serialization() {
        // give payload
        let payload = create_test_event_payload();
        // create event
        let event = Event::new(
            &EventType::BillSigned,
            "peer_id".to_string(),
            payload.clone(),
        );
        // create envelope
        let envelope: EventEnvelope = event.clone().try_into().unwrap();

        // check that the envelope is correct
        assert_eq!(
            &event.event_type, &envelope.event_type,
            "envelope has wrong event type"
        );
        assert_eq!(
            &event.peer_id, &envelope.peer_id,
            "envelope has wrong peer id"
        );

        // check that the deserialization works
        let deserialized_event: Event<test_utils::TestEventPayload> = envelope.try_into().unwrap();
        assert_eq!(
            &deserialized_event.data, &payload,
            "payload was not deserialized correctly"
        );
        assert_eq!(
            &deserialized_event.event_type, &event.event_type,
            "deserialized event has wrong event type"
        );
        assert_eq!(
            &deserialized_event.peer_id, &event.peer_id,
            "deserialized event has wrong peer id"
        );
    }

    #[tokio::test]
    async fn test_event_handling() {
        let accepted_event = EventType::BillPaid;

        // given a handler that accepts the event type
        let event_handler: TestEventHandler<TestEventPayload> =
            TestEventHandler::new(Some(accepted_event.to_owned()));

        // event type should be accepted
        assert!(event_handler.handles_event(&accepted_event));

        // given an event and encode it to an envelope
        let event = create_test_event(&EventType::BillPaid);
        let envelope: EventEnvelope = event.clone().try_into().unwrap();

        // handler should run successfully
        event_handler
            .handle_event(envelope)
            .await
            .expect("event was not handled");

        // handler should have been invoked
        let called = event_handler.called.lock().await;
        assert!(*called, "event was not handled");

        // and the event should have been received
        let received = event_handler.received_event.lock().await.clone().unwrap();
        assert_eq!(event.data, received.data, "handled payload was not correct");
    }
}

/// These mocks might be useful for testing in other modules as well
#[cfg(test)]
pub mod test_utils {

    use serde::Deserialize;
    use tokio::sync::Mutex;

    use super::*;

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
}
