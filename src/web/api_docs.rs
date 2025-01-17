use super::handlers;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "BitCredit E-Bills API",
        description = "Allows to execute operations and monitor state of a BitCredit E-Bill instance",
        version = "1.0.0",
    ),
    paths(
        handlers::notifications::list_notifications,
        handlers::notifications::mark_notification_done,
        handlers::notifications::websocket,
        handlers::notifications::sse,
        handlers::bill::return_bills_list,
        handlers::bill::return_bill,
        handlers::identity::return_identity,
        handlers::identity::create_identity,
        handlers::identity::change_identity,
        handlers::identity::get_seed_phrase,
        handlers::identity::recover_from_seed_phrase,
    )
)]
pub struct ApiDocs;
