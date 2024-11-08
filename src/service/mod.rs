pub mod contact_service;
pub mod notification_service;

use super::{dht::Client, Config};
use crate::persistence;
use crate::persistence::FileBasedContactStore;
use contact_service::{ContactService, ContactServiceApi};
use log::error;
use rocket::{http::Status, response::Responder};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::broadcast;

/// Generic result type
pub type Result<T> = std::result::Result<T, Error>;

/// Generic error type
#[derive(Debug, Error)]
pub enum Error {
    /// all errors originating from the persistence layer
    #[error("Persistence error: {0}")]
    Persistence(#[from] persistence::Error),

    /// errors that currently return early http status code Status::NotAcceptable
    #[error("not acceptable")]
    PreconditionFailed,

    /// errors stemming from json deserialization. Most of the time this is a
    /// bad request on the api but can also be caused by deserializing other messages
    #[error("Deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// errors stemming from sending or receiving notifications
    #[error("Notification service error: {0}")]
    NotificationService(#[from] notification_service::Error),
}

/// Map from service errors directly to rocket status codes. This allows us to
/// write handlers that return `Result<T, service::Error>` and still return the correct
/// status code.
impl<'r, 'o: 'r> Responder<'r, 'o> for Error {
    fn respond_to(self, req: &rocket::Request) -> rocket::response::Result<'o> {
        match self {
            // for now handle all persistence errors as InternalServerError, there
            // will be cases where we want to handle them differently (eg. 409 Conflict)
            Error::Persistence(_) => Status::InternalServerError.respond_to(req),
            Error::PreconditionFailed => Status::NotAcceptable.respond_to(req),
            Error::Json(_) => Status::BadRequest.respond_to(req),
            Error::NotificationService(_) => Status::InternalServerError.respond_to(req),
        }
    }
}

/// A dependency container for all services that are used by the application
#[derive(Clone)]
pub struct ServiceContext {
    pub config: Config,
    dht_client: Client,
    pub contact_service: Arc<dyn ContactServiceApi>,
    pub shutdown_sender: broadcast::Sender<bool>,
}

impl ServiceContext {
    pub fn new(
        config: Config,
        client: Client,
        contact_service: ContactService,
        shutdown_sender: broadcast::Sender<bool>,
    ) -> Self {
        Self {
            config,
            dht_client: client,
            contact_service: Arc::new(contact_service),
            shutdown_sender,
        }
    }

    /// returns an owned instance of the dht client
    pub fn dht_client(&self) -> Client {
        self.dht_client.clone()
    }

    /// sends a shutdown event to all parts of the application
    pub fn shutdown(&self) {
        if let Err(e) = self.shutdown_sender.send(true) {
            error!("Error sending shutdown event: {e}");
        }
    }
}

/// building up the service context dependencies here for now. Later we can modularize this
/// and make it more flexible.
pub async fn create_service_context(
    config: Config,
    client: Client,
    shutdown_sender: broadcast::Sender<bool>,
) -> Result<ServiceContext> {
    let contact_store = FileBasedContactStore::new(&config.data_dir, "contacts", "contacts")?;
    let contact_service = ContactService::new(client.clone(), Arc::new(contact_store));

    Ok(ServiceContext::new(
        config,
        client,
        contact_service,
        shutdown_sender,
    ))
}
