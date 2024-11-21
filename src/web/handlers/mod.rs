use crate::blockchain::OperationCode;
use crate::service::ServiceContext;
use rocket::serde::json::Json;
use rocket::{get, Shutdown, State};

pub mod bill;
pub mod company;
pub mod contacts;
pub mod identity;
pub mod quotes;

#[get("/")]
pub async fn exit(shutdown: Shutdown, state: &State<ServiceContext>) {
    log::info!("Exit called - shutting down...");
    shutdown.notify();
    state.shutdown();
}

#[get("/return")]
pub async fn return_operation_codes() -> Json<Vec<OperationCode>> {
    Json(OperationCode::get_all_operation_codes())
}
