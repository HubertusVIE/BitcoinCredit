use super::{EventEnvelope, Result};
use async_trait::async_trait;
use log::info;

#[async_trait]
pub trait NotificationTransportApi: Send + Sync {
    async fn send(&self, event: EventEnvelope) -> Result<()>;
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
