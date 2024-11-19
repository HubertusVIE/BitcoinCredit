use super::super::data::{DeleteContactForm, EditContactForm, NewContactForm};
use crate::service::contact_service::Contact;
use crate::service::{self, Result, ServiceContext};
use rocket::form::Form;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{delete, get, post, put, State};

#[get("/return")]
pub async fn return_contacts(state: &State<ServiceContext>) -> Result<Json<Vec<Contact>>> {
    let contacts: Vec<Contact> = state.contact_service.get_contacts().await?;
    Ok(Json(contacts))
}

#[delete("/remove", data = "<remove_contact_form>")]
pub async fn remove_contact(
    remove_contact_form: Form<DeleteContactForm>,
    state: &State<ServiceContext>,
) -> Result<Status> {
    if !state.identity_service.identity_exists().await {
        return Err(service::Error::PreconditionFailed);
    }
    state
        .contact_service
        .delete_identity_by_name(&remove_contact_form.name)
        .await?;
    Ok(Status::Ok)
}

#[post("/new", data = "<new_contact_form>")]
pub async fn new_contact(
    state: &State<ServiceContext>,
    new_contact_form: Form<NewContactForm>,
) -> Result<Json<Vec<Contact>>> {
    if !state.identity_service.identity_exists().await {
        return Err(service::Error::PreconditionFailed);
    }
    state
        .contact_service
        .add_peer_identity(&new_contact_form.name, &new_contact_form.node_id)
        .await?;
    let res = state.contact_service.get_contacts().await?;
    Ok(Json(res))
}

#[put("/edit", data = "<edit_contact_form>")]
pub async fn edit_contact(
    edit_contact_form: Form<EditContactForm>,
    state: &State<ServiceContext>,
) -> Result<Json<Vec<Contact>>> {
    if !state.identity_service.identity_exists().await {
        return Err(service::Error::PreconditionFailed);
    }
    state
        .contact_service
        .update_identity_name(&edit_contact_form.old_name, &edit_contact_form.name)
        .await?;
    let res = state.contact_service.get_contacts().await?;
    Ok(Json(res))
}
