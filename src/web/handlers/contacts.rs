use super::super::data::{EditContactPayload, NewContactPayload};
use crate::service::contact_service::Contact;
use crate::service::{self, Result, ServiceContext};
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{delete, get, post, put, State};

#[get("/return")]
pub async fn return_contacts(state: &State<ServiceContext>) -> Result<Json<Vec<Contact>>> {
    let contacts: Vec<Contact> = state.contact_service.get_contacts().await?;
    Ok(Json(contacts))
}

#[delete("/remove/<contact_name>")]
pub async fn remove_contact(state: &State<ServiceContext>, contact_name: &str) -> Result<Status> {
    if !state.identity_service.identity_exists().await {
        return Err(service::Error::PreconditionFailed);
    }
    state
        .contact_service
        .delete_identity_by_name(contact_name)
        .await?;
    Ok(Status::Ok)
}

#[post("/new", format = "json", data = "<new_contact_payload>")]
pub async fn new_contact(
    state: &State<ServiceContext>,
    new_contact_payload: Json<NewContactPayload>,
) -> Result<Json<Vec<Contact>>> {
    if !state.identity_service.identity_exists().await {
        return Err(service::Error::PreconditionFailed);
    }
    state
        .contact_service
        .add_peer_identity(&new_contact_payload.name, &new_contact_payload.node_id)
        .await?;
    let res = state.contact_service.get_contacts().await?;
    Ok(Json(res))
}

#[put("/edit", format = "json", data = "<edit_contact_payload>")]
pub async fn edit_contact(
    state: &State<ServiceContext>,
    edit_contact_payload: Json<EditContactPayload>,
) -> Result<Json<Vec<Contact>>> {
    if !state.identity_service.identity_exists().await {
        return Err(service::Error::PreconditionFailed);
    }
    state
        .contact_service
        .update_identity_name(&edit_contact_payload.old_name, &edit_contact_payload.name)
        .await?;
    let res = state.contact_service.get_contacts().await?;
    Ok(Json(res))
}
