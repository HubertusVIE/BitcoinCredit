use super::Result;
use async_trait::async_trait;
use serde_json::Value;
use serde::{Serialize, Deserialize};
use surrealdb::{engine::any::Any, sql::Thing, Surreal};

use crate::{
    persistence::notification::NotificationStoreApi,
    service::notification_service::{Notification, NotificationType},
};

#[derive(Clone)]
pub struct SurrealNotificationStore {
    db: Surreal<Any>,
}

impl SurrealNotificationStore {
    const TABLE: &'static str = "notifications";

    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl NotificationStoreApi for SurrealNotificationStore {
    /// Stores a new notification into the database
    async fn add_notification(&self, notification: Notification) -> Result<()> {
        todo!()
    }
    /// Returns all currently active notifications from the database
    async fn list_notifications(&self) -> Result<Vec<Notification>> {
        todo!()
    }
    /// Returns the latest active notification for the given reference and notification type
    async fn get_notifiction_by_reference(
        &self,
        reference: &str,
        notification_type: NotificationType,
    ) -> Result<Option<Notification>> {
        todo!()
    }
    /// Returns all notifications for the given reference and notification type that are active
    async fn get_notifications_by_type(
        &self,
        notification_type: NotificationType,
    ) -> Result<Vec<Notification>> {
        todo!()
    }
    /// Marks an active notification as done
    async fn mark_notification_as_done(&self, notification_id: &str) -> Result<()> {
        todo!()
    }
    /// deletes a notification from the database
    async fn delete_notification(&self, notification_id: &str) -> Result<()> {
        todo!()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NotificationDb {
    id: Thing,
    notification_type: NotificationType,
    reference_id: Option<String>,
    description: String,
    timestamp: u64,
    active: bool,
    payload: Option<Value>,
}
