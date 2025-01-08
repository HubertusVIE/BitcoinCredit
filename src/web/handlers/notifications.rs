use crate::service::notification_service::Notification;
use crate::service::{Result, ServiceContext};
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{get, post, State};

#[get("/")]
pub async fn list_notifications(state: &State<ServiceContext>) -> Result<Json<Vec<Notification>>> {
    let notifications: Vec<Notification> = state
        .notification_service
        .get_client_notifications()
        .await?;
    Ok(Json(notifications))
}

#[post("/done/<notification_id>")]
pub async fn mark_notification_done(
    state: &State<ServiceContext>,
    notification_id: &str,
) -> Result<Status> {
    state
        .notification_service
        .mark_notification_as_done(notification_id)
        .await?;
    Ok(Status::Ok)
}
