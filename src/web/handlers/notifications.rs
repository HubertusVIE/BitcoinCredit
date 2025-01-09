use crate::service::notification_service::Notification;
use crate::service::{Result, ServiceContext};
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{get, post, State};

#[utoipa::path(
    tag = "notifications",
    description = "Get all active notifications",
    responses(
        (status = 200, description = "List of notifications", body = Vec<Notification>)
    )
)]
#[get("/notifications")]
pub async fn list_notifications(state: &State<ServiceContext>) -> Result<Json<Vec<Notification>>> {
    let notifications: Vec<Notification> = state
        .notification_service
        .get_client_notifications()
        .await?;
    Ok(Json(notifications))
}

#[utoipa::path(
    tag = "notifications",
    description = "Marks a notification as done",
    params(
        ("notification_id" = String, description = "Id of the notification to marks as done")
    ),
    responses(
        (status = 200, description = "Notification set to done")
    )
)]
#[post("/notifications/<notification_id>/done")]
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
