use super::Result;
use async_trait::async_trait;
use log::info;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

/// The different types of events that can be sent via this service.
/// For now we only have Bill events and this needs some clippy
/// exceptions here. As soon as we have other event topics, we can
/// add new types here and remove the clippy exceptions.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
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

/// A generic event that can be sent to a specific recipient
/// and is serializable. The recipient is currently just a string,
/// and we have to decide what the identifier is.
/// This event should contain all the information that is needed
/// to send to different channels including email, push and Nostr.
#[derive(Serialize, Debug, Clone)]
pub struct Event<T: Serialize> {
    pub event_type: EventType,
    pub recipient: String,
    pub data: T,
}

impl<T: Serialize> Event<T> {
    #[allow(dead_code)]
    pub fn new(event_type: EventType, recipient: String, data: T) -> Self {
        Self {
            event_type,
            recipient,
            data,
        }
    }
}

/// When we receive an event, we need to know what type it is and
/// how to handle it. This payload envelope allows us to find out
/// the type of event to later deserialize the data into the correct
/// type.
#[derive(Serialize, Debug, Clone)]
pub struct EventEnvelope {
    pub event_type: EventType,
    pub recipient: String,
    pub data: Value,
}

impl<T: Serialize> TryFrom<Event<T>> for EventEnvelope {
    type Error = super::Error;

    fn try_from(event: Event<T>) -> Result<Self> {
        Ok(Self {
            event_type: event.event_type,
            recipient: event.recipient,
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
            recipient: envelope.recipient,
            data,
        })
    }
}

/// Send an event via all channels required for the event type.
#[allow(dead_code)]
#[async_trait]
pub trait NotificationServiceApi: Send + Sync {
    async fn send_event<T: Serialize>(&self, event: Event<T>) -> Result<()>;
}

/// Handle an event when we receive it from a channel.
#[allow(dead_code)]
#[async_trait]
pub trait NotificationHandlerApi: Send + Sync {
    /// Whether this handler handles the given event type.
    fn handles_event(&self, event_type: EventType) -> bool;

    /// Handle the event. This is called by the notification processor which should
    /// have checked the event type before calling this method. The actual implementation
    /// should be able to deserialize the data into its T type because the EventType
    /// determines the T type.
    async fn handle_event(&self, event: EventEnvelope) -> Result<()>;
}

/// Logs all events that are received and registered in the event_types.
pub struct LoggingEventHandler {
    event_types: Vec<EventType>,
}

/// Just a dummy handler that logs the event and returns Ok(())
#[async_trait]
impl NotificationHandlerApi for LoggingEventHandler {
    fn handles_event(&self, event_type: EventType) -> bool {
        self.event_types.contains(&event_type)
    }

    async fn handle_event(&self, event: EventEnvelope) -> Result<()> {
        info!("Received event: {event:?}");
        Ok(())
    }
}
