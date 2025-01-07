use super::super::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use surrealdb::{engine::any::Any, sql::Thing, Surreal};

use crate::{
    persistence::notification::NotificationStoreApi,
    service::notification_service::{Notification, NotificationType},
    util::date::DateTimeUtc,
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
    async fn add(&self, notification: Notification) -> Result<Notification> {
        let id = notification.id.to_owned();
        let entity: NotificationDb = notification.into();
        let result: Option<NotificationDb> = self
            .db
            .insert((Self::TABLE, id.to_string()))
            .content(entity)
            .await?;

        match result {
            Some(n) => Ok(n.into()),
            None => Err(Error::InsertFailed(format!(
                "{} with id {}",
                Self::TABLE,
                id
            ))),
        }
    }
    /// Returns all currently active notifications from the database
    async fn list(&self) -> Result<Vec<Notification>> {
        let result: Vec<NotificationDb> = self
            .db
            .query("SELECT * FROM type::table($table) WHERE active = true ORDER BY datetime DESC")
            .bind(("table", Self::TABLE))
            .await?
            .take(0)?;

        Ok(result.into_iter().map(|n| n.into()).collect())
    }
    /// Returns the latest active notification for the given reference and notification type
    async fn get_latest_by_reference(
        &self,
        reference: &str,
        notification_type: NotificationType,
    ) -> Result<Option<Notification>> {
        let result: Vec<NotificationDb> = self.db.query("SELECT * FROM type::table($table) WHERE active = true AND reference_id = $reference_id AND notification_type = $notification_type ORDER BY datetime desc")
            .bind(("table", Self::TABLE))
            .bind(("reference_id", reference.to_owned()))
            .bind(("notification_type", notification_type))
            .await?
            .take(0)?;

        Ok(result.first().map(|n| n.clone().into()))
    }
    /// Returns all notifications for the given reference and notification type that are active
    async fn list_by_type(&self, notification_type: NotificationType) -> Result<Vec<Notification>> {
        let result: Vec<NotificationDb> = self.db.query("SELECT * FROM type::table($table) WHERE active = true AND notification_type = $notification_type ORDER BY datetime desc")
            .bind(("table", Self::TABLE))
            .bind(("notification_type", notification_type))
            .await?
        .take(0)?;

        Ok(result.into_iter().map(|n| n.into()).collect())
    }
    /// Marks an active notification as done
    async fn mark_as_done(&self, notification_id: &str) -> Result<()> {
        let thing: Thing = (Self::TABLE, notification_id).into();
        self.db
            .query("UPDATE $id SET active = false")
            .bind(("id", thing))
            .await?;
        Ok(())
    }
    /// deletes a notification from the database
    async fn delete(&self, notification_id: &str) -> Result<()> {
        let _: Option<NotificationDb> = self.db.delete((Self::TABLE, notification_id)).await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NotificationDb {
    pub id: Thing,
    pub notification_type: NotificationType,
    pub reference_id: Option<String>,
    pub description: String,
    pub datetime: DateTimeUtc,
    pub active: bool,
    pub payload: Option<Value>,
}

impl From<NotificationDb> for Notification {
    fn from(value: NotificationDb) -> Self {
        Self {
            id: value.id.id.to_raw(),
            notification_type: value.notification_type,
            reference_id: value.reference_id,
            description: value.description,
            datetime: value.datetime,
            active: value.active,
            payload: value.payload,
        }
    }
}

impl From<Notification> for NotificationDb {
    fn from(value: Notification) -> Self {
        Self {
            id: (
                SurrealNotificationStore::TABLE.to_owned(),
                value.id.to_owned(),
            )
                .into(),
            notification_type: value.notification_type,
            reference_id: value.reference_id,
            description: value.description,
            datetime: value.datetime,
            active: value.active,
            payload: value.payload,
        }
    }
}

#[cfg(test)]
mod tests {

    use serde_json::json;
    use uuid::Uuid;

    use super::*;
    use crate::{persistence::db::get_memory_db, util::date::now};

    async fn get_store() -> SurrealNotificationStore {
        let db = get_memory_db("test", "notification")
            .await
            .expect("could not create memory db");
        SurrealNotificationStore::new(db)
    }

    #[tokio::test]
    async fn test_inserts_and_queries_notifiction() {
        let store = get_store().await;
        let notification = test_notification("bill_id", Some(test_payload()));
        let r = store
            .add(notification.clone())
            .await
            .expect("could not create notification");

        let all = store.list().await.expect("could not list notifications");
        assert!(!all.is_empty());
        assert_eq!(notification.id, r.id);
    }

    #[tokio::test]
    async fn test_deletes_existing_notifiction() {
        let store = get_store().await;
        let notification = test_notification("bill_id", Some(test_payload()));
        let r = store
            .add(notification.clone())
            .await
            .expect("could not create notification");

        let all = store.list().await.expect("could not list notifications");
        assert!(!all.is_empty());

        store
            .delete(&r.id)
            .await
            .expect("could not delete notification");
        let all = store.list().await.expect("could not list notifications");
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn test_marks_done_and_no_longer_returns_in_list() {
        let store = get_store().await;
        let notification = test_notification("bill_id", Some(test_payload()));
        let r = store
            .add(notification.clone())
            .await
            .expect("could not create notification");

        let all = store.list().await.expect("could not list notifications");
        assert!(!all.is_empty());

        store
            .mark_as_done(&r.id)
            .await
            .expect("could not mark notification as done");

        let all = store.list().await.expect("could not list notifications");
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn test_marks_done_and_no_longer_returns_by_reference() {
        let store = get_store().await;
        let notification = test_notification("bill_id", Some(test_payload()));
        let r = store
            .add(notification.clone())
            .await
            .expect("could not create notification");

        let latest = store
            .get_latest_by_reference(
                &notification.clone().reference_id.unwrap(),
                NotificationType::Bill,
            )
            .await
            .expect("could not list notifications");
        assert!(latest.is_some());

        store
            .mark_as_done(&r.id)
            .await
            .expect("could not mark notification as done");

        let latest = store
            .get_latest_by_reference(
                &notification.clone().reference_id.unwrap(),
                NotificationType::Bill,
            )
            .await
            .expect("could not list notifications");

        assert!(latest.is_none());
    }

    #[tokio::test]
    async fn test_returns_all_active_by_type() {
        let store = get_store().await;
        let notification1 = test_notification("bill_id1", Some(test_payload()));
        let notification2 = test_notification("bill_id2", Some(test_payload()));
        let notification3 = test_general_notification();
        let _ = store
            .add(notification1.clone())
            .await
            .expect("notification created");
        let _ = store
            .add(notification2.clone())
            .await
            .expect("notification created");
        let _ = store
            .add(notification3.clone())
            .await
            .expect("notification created");
        store
            .mark_as_done(&notification2.clone().id)
            .await
            .expect("notification marked done");
        let by_type = store
            .list_by_type(NotificationType::Bill)
            .await
            .expect("returned list by type");

        assert_eq!(by_type.len(), 1, "should only have one bill type in list");
        by_type.iter().for_each(|n| {
            assert!(n.active);
            assert_ne!(
                n.id, notification2.id,
                "notfication 2 should be done already"
            );
        });
    }

    fn test_notification(bill_id: &str, payload: Option<Value>) -> Notification {
        Notification::new_bill_notification(bill_id, "test_notification", payload)
    }

    fn test_payload() -> Value {
        json!({ "Some": "value", "for": 66, "testing": true })
    }

    fn test_general_notification() -> Notification {
        Notification {
            id: Uuid::new_v4().to_string(),
            notification_type: NotificationType::General,
            reference_id: Some("general".to_string()),
            description: "general desc".to_string(),
            datetime: now(),
            active: true,
            payload: None,
        }
    }
}
