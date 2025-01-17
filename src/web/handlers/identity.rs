use super::middleware::IdentityCheck;
use crate::external;
use crate::service::Result;
use crate::web::data::{ChangeIdentityPayload, IdentityPayload, SeedPhrase, SwitchIdentity};
use crate::{service::identity_service::IdentityToReturn, service::ServiceContext};
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{get, post, put, State};

#[utoipa::path(
    tag = "Identity",
    path = "/identity/return",
    description = "Returns the current identity",
    responses(
        (status = 200, description = "The current identity data", body = IdentityToReturn)
    ),
)]
#[get("/return")]
pub async fn return_identity(state: &State<ServiceContext>) -> Result<Json<IdentityToReturn>> {
    let my_identity = if !state.identity_service.identity_exists().await {
        return Err(crate::service::Error::NotFound);
    } else {
        let full_identity = state.identity_service.get_full_identity().await?;
        IdentityToReturn::from(full_identity.identity, full_identity.key_pair)?
    };
    Ok(Json(my_identity))
}

#[utoipa::path(
    tag = "Identity",
    path = "/identity/create",
    description = "Creates a new identity with given data",
    responses(
        (status = 200, description = "The identity has been created")
    ),
    request_body(description = "The data to create an identity with", content((IdentityPayload)))
)]
#[post("/create", format = "json", data = "<identity_payload>")]
pub async fn create_identity(
    state: &State<ServiceContext>,
    identity_payload: Json<IdentityPayload>,
) -> Result<Status> {
    let identity = identity_payload.into_inner();
    let timestamp = external::time::TimeApi::get_atomic_time().await.timestamp;
    state
        .identity_service
        .create_identity(
            identity.name,
            identity.date_of_birth,
            identity.city_of_birth,
            identity.country_of_birth,
            identity.email,
            identity.postal_address,
            timestamp,
        )
        .await?;
    Ok(Status::Ok)
}

#[utoipa::path(
    tag = "Identity",
    path = "/identity/change",
    description = "Updates the identity with given data",
    responses(
        (status = 200, description = "The identity has been updated")
    ),
    request_body(description = "The data to update identity with", content((ChangeIdentityPayload)))
)]
#[put("/change", format = "json", data = "<identity_payload>")]
pub async fn change_identity(
    _identity: IdentityCheck,
    state: &State<ServiceContext>,
    identity_payload: Json<ChangeIdentityPayload>,
) -> Result<Status> {
    let identity_payload = identity_payload.into_inner();
    if identity_payload.name.is_none()
        && identity_payload.email.is_none()
        && identity_payload.postal_address.is_none()
    {
        return Ok(Status::Ok);
    }
    let timestamp = external::time::TimeApi::get_atomic_time().await.timestamp;
    state
        .identity_service
        .update_identity(
            identity_payload.name,
            identity_payload.email,
            identity_payload.postal_address,
            timestamp,
        )
        .await?;
    Ok(Status::Ok)
}

#[utoipa::path(
    tag = "Identity",
    path = "/identity/active",
    description = "Returns the currently active identity data",
    responses(
        (status = 200, description = "The identity that is currently active", body = SwitchIdentity)
    )
)]
#[get("/active")]
pub async fn active(state: &State<ServiceContext>) -> Result<Json<SwitchIdentity>> {
    let current_identity_state = state.get_current_identity().await;
    let node_id = match current_identity_state.company {
        None => current_identity_state.personal,
        Some(company_node_id) => company_node_id,
    };
    Ok(Json(SwitchIdentity { node_id }))
}

#[utoipa::path(
    tag = "Identity",
    path = "/identity/switch",
    description = "Switches the currently active identity to the given identity",
    responses(
        (status = 200, description = "The active identity has been switched")
    ),
    request_body(description = "The identity identifier to switch to", content((SwitchIdentity)))
)]
#[put("/switch", format = "json", data = "<switch_identity_payload>")]
pub async fn switch(
    state: &State<ServiceContext>,
    switch_identity_payload: Json<SwitchIdentity>,
) -> Result<Status> {
    let node_id = switch_identity_payload.0.node_id;
    let personal_node_id = state.identity_service.get_identity().await?.node_id;

    // if it's the personal node id, set it
    if node_id == personal_node_id {
        state.set_current_personal_identity(node_id).await;
        return Ok(Status::Ok);
    }

    // if it's one of our companies, set it
    if state
        .company_service
        .get_list_of_companies()
        .await?
        .iter()
        .any(|c| c.id == node_id)
    {
        state.set_current_company_identity(node_id).await;
        return Ok(Status::Ok);
    }

    // otherwise, return an error
    Err(crate::service::Error::Validation(format!(
        "The provided node_id: {node_id} is not a valid company id, or personal node_id"
    )))
}

#[utoipa::path(
    tag = "Identity",
    path = "/identity/seed/backup",
    description = "Returns the seed phrase key backup of current private key",
    responses(
        (status = 200, description = "The seed phrase of the current private key", body = SeedPhrase)
    )
)]
#[get("/seed/backup")]
pub async fn get_seed_phrase(state: &State<ServiceContext>) -> Result<Json<SeedPhrase>> {
    let seed_phrase = state.identity_service.get_seedphrase().await?;
    Ok(Json(SeedPhrase { seed_phrase }))
}

#[utoipa::path(
    tag = "Identity",
    path = "/identity/seed/recover",
    description = "Restores a private key from the given seed phrase backup",
    responses(
        (status = 200, description = "Private key has been recovered from seed")
    ),
    request_body(description = "The seed phrase to recover the private key from", content((SeedPhrase)))
)]
#[put("/seed/recover", format = "json", data = "<payload>")]
pub async fn recover_from_seed_phrase(
    state: &State<ServiceContext>,
    payload: Json<SeedPhrase>,
) -> Result<Status> {
    state
        .identity_service
        .recover_from_seedphrase(&payload.into_inner().seed_phrase)
        .await?;
    Ok(Status::Ok)
}
