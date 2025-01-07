use async_trait::async_trait;

use super::Result;
use crate::service::notification_service::{Notification, NotificationType};

#[async_trait]
pub trait NotificationStoreApi {
    /// Stores a new notification into the database
    async fn add_notification(&self, notification: Notification) -> Result<()>;
    /// Returns all currently active notifications from the database
    async fn list_notifications(&self) -> Result<Vec<Notification>>;
    /// Returns the latest active notification for the given reference and notification type
    async fn get_notifiction_by_reference(
        &self,
        reference: &str,
        notification_type: NotificationType,
    ) -> Result<Option<Notification>>;
    /// Returns all notifications for the given reference and notification type that are active
    async fn get_notifications_by_type(
        &self,
        notification_type: NotificationType,
    ) -> Result<Vec<Notification>>;
    /// Marks an active notification as done
    async fn mark_notification_as_done(&self, notification_id: &str) -> Result<()>;
    /// deletes a notification from the database
    async fn delete_notification(&self, notification_id: &str) -> Result<()>;
}
