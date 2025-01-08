use async_trait::async_trait;

use super::Result;
use crate::service::notification_service::{Notification, NotificationType};
#[cfg(test)]
use mockall::automock;

#[cfg_attr(test, automock)]
#[async_trait]
pub trait NotificationStoreApi: Send + Sync {
    /// Stores a new notification into the database
    async fn add(&self, notification: Notification) -> Result<Notification>;
    /// Returns all currently active notifications from the database
    async fn list(&self) -> Result<Vec<Notification>>;
    /// Returns the latest active notification for the given reference and notification type
    async fn get_latest_by_reference(
        &self,
        reference: &str,
        notification_type: NotificationType,
    ) -> Result<Option<Notification>>;
    /// Returns all notifications for the given reference and notification type that are active
    async fn list_by_type(&self, notification_type: NotificationType) -> Result<Vec<Notification>>;
    /// Marks an active notification as done
    async fn mark_as_done(&self, notification_id: &str) -> Result<()>;
    /// deletes a notification from the database
    async fn delete(&self, notification_id: &str) -> Result<()>;
}
