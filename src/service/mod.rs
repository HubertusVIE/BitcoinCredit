pub mod bill_service;
pub mod contact_service;
pub mod identity_service;
pub mod notification_service;

use super::{dht::Client, Config};
use crate::external;
use crate::persistence::DbContext;
use crate::persistence::{self};
use bill_service::{BillService, BillServiceApi};
use contact_service::{ContactService, ContactServiceApi};
use identity_service::{IdentityService, IdentityServiceApi};
use log::error;
use rocket::http::ContentType;
use rocket::Response;
use rocket::{http::Status, response::Responder};
use serde_json::json;
use std::io::Cursor;
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
    #[error("JSON Serialization/Deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// errors stemming from sending or receiving notifications
    #[error("Notification service error: {0}")]
    NotificationService(#[from] notification_service::Error),

    /// errors stemming from cryptography, such as converting keys
    #[error("Cryptography error: {0}")]
    Cryptography(String),

    /// errors stemming from interaction with the DHT
    #[error("DHT error: {0}")]
    Dht(String),

    /// errors that stem from validation
    #[error("Validation Error: {0}")]
    Validation(String),

    #[error("External API error: {0}")]
    ExternalApi(#[from] external::Error),
}

/// Map from service errors directly to rocket status codes. This allows us to
/// write handlers that return `Result<T, service::Error>` and still return the correct
/// status code.
impl<'r, 'o: 'r> Responder<'r, 'o> for Error {
    fn respond_to(self, req: &rocket::Request) -> rocket::response::Result<'o> {
        match self {
            // for now, DHT errors are InternalServerError
            Error::Dht(e) => {
                error!("{e}");
                Status::InternalServerError.respond_to(req)
            }
            // for now, Cryptography errors are InternalServerError
            Error::Cryptography(e) => {
                error!("{e}");
                Status::InternalServerError.respond_to(req)
            }
            // for now handle all persistence errors as InternalServerError, there
            // will be cases where we want to handle them differently (eg. 409 Conflict)
            Error::Persistence(e) => {
                error!("{e}");
                Status::InternalServerError.respond_to(req)
            }
            Error::Json(e) => {
                error!("{e}");
                Status::BadRequest.respond_to(req)
            }
            Error::PreconditionFailed => Status::NotAcceptable.respond_to(req),
            Error::NotificationService(_) => Status::InternalServerError.respond_to(req),
            Error::Validation(msg) => {
                let body = json!({ "error": "validation_error", "message": msg }).to_string();
                Response::build()
                    .status(Status::BadRequest)
                    .header(ContentType::JSON)
                    .sized_body(body.len(), Cursor::new(body))
                    .ok()
            }
            // If an external API errors, we can only tell the caller that something went wrong on
            // our end
            Error::ExternalApi(e) => {
                error!("{e}");
                Status::InternalServerError.respond_to(req)
            }
        }
    }
}

/// A dependency container for all services that are used by the application
#[derive(Clone)]
pub struct ServiceContext {
    pub config: Config,
    dht_client: Client,
    pub contact_service: Arc<dyn ContactServiceApi>,
    pub bill_service: Arc<dyn BillServiceApi>,
    pub identity_service: Arc<dyn IdentityServiceApi>,
    pub shutdown_sender: broadcast::Sender<bool>,
}

impl ServiceContext {
    pub fn new(
        config: Config,
        client: Client,
        contact_service: ContactService,
        bill_service: BillService,
        identity_service: IdentityService,
        shutdown_sender: broadcast::Sender<bool>,
    ) -> Self {
        Self {
            config,
            dht_client: client,
            contact_service: Arc::new(contact_service),
            bill_service: Arc::new(bill_service),
            identity_service: Arc::new(identity_service),
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
    db: DbContext,
) -> Result<ServiceContext> {
    let contact_service = ContactService::new(client.clone(), db.contact_store);
    let bill_service = BillService::new(client.clone(), db.bill_store, db.identity_store.clone());
    let identity_service = IdentityService::new(client.clone(), db.identity_store);

    Ok(ServiceContext::new(
        config,
        client,
        contact_service,
        bill_service,
        identity_service,
        shutdown_sender,
    ))
}
