use crate::service::ServiceContext;
use rocket::{get, Shutdown, State};

pub mod bill;
pub mod company;
pub mod contacts;
pub mod identity;
pub mod notifications;
pub mod quotes;

#[get("/")]
pub async fn exit(shutdown: Shutdown, state: &State<ServiceContext>) {
    log::info!("Exit called - shutting down...");
    shutdown.notify();
    state.shutdown();
}
