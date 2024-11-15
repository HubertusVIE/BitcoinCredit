use super::super::data::IdentityForm;
use crate::service::{self, Result};
use crate::web::data::ChangeIdentityForm;
use crate::{
    bill::identity::{Identity, NodeId},
    service::ServiceContext,
};
use libp2p::PeerId;
use rocket::form::Form;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{get, post, put, State};

#[get("/return")]
pub async fn return_identity(state: &State<ServiceContext>) -> Result<Json<Identity>> {
    let my_identity = if !state.identity_service.identity_exists().await {
        Identity::new_empty()
    } else {
        state.identity_service.get_identity().await?
    };
    Ok(Json(my_identity))
}

#[get("/peer_id/return")]
pub async fn return_peer_id(state: &State<ServiceContext>) -> Result<Json<NodeId>> {
    let peer_id: PeerId = state.identity_service.get_peer_id().await?;
    let node_id = NodeId::new(peer_id.to_string());
    Ok(Json(node_id))
}

#[post("/create", data = "<identity_form>")]
pub async fn create_identity(
    identity_form: Form<IdentityForm>,
    state: &State<ServiceContext>,
) -> Result<Status> {
    let identity: IdentityForm = identity_form.into_inner();
    state
        .identity_service
        .create_identity(
            identity.name,
            identity.company,
            identity.date_of_birth,
            identity.city_of_birth,
            identity.country_of_birth,
            identity.email,
            identity.postal_address,
        )
        .await?;
    Ok(Status::Ok)
}

#[put("/change", data = "<identity_form>")]
pub async fn change_identity(
    identity_form: Form<ChangeIdentityForm>,
    state: &State<ServiceContext>,
) -> Result<Status> {
    let identity_form = identity_form.into_inner();
    let mut identity_changes: Identity = Identity::new_empty();
    identity_changes.name = identity_form.name.trim().to_string();
    identity_changes.company = identity_form.company.trim().to_string();
    identity_changes.email = identity_form.email.trim().to_string();
    identity_changes.postal_address = identity_form.postal_address.trim().to_string();

    let mut my_identity: Identity;
    if !state.identity_service.identity_exists().await {
        return Err(service::Error::PreconditionFailed);
    }
    my_identity = state.identity_service.get_identity().await?;

    if !my_identity.update_valid(&identity_changes) {
        return Err(service::Error::PreconditionFailed);
    }
    my_identity.update_from(&identity_changes);

    state.identity_service.update_identity(&my_identity).await?;

    Ok(Status::Ok)
}
