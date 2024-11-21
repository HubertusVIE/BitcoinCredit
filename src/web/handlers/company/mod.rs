use crate::{
    service::{Error, Result, ServiceContext},
    util::file::UploadFileHandler,
    web::data::{UploadFileForm, UploadFilesResponse},
};
use data::{AddSignatoryPayload, CreateCompanyPayload, EditCompanyPayload, RemoveSignatoryPayload};
use rocket::{form::Form, get, post, put, serde::json::Json, State};

pub mod data;

#[get("/list")]
pub async fn list(state: &State<ServiceContext>) -> Result<Json<Vec<data::CompanyToReturn>>> {
    Ok(Json(vec![]))
}

#[post("/upload_file", data = "<file_upload_form>")]
pub async fn upload_file(
    state: &State<ServiceContext>,
    file_upload_form: Form<UploadFileForm<'_>>,
) -> Result<Json<UploadFilesResponse>> {
    if !state.identity_service.identity_exists().await {
        return Err(Error::PreconditionFailed);
    }

    let file = &file_upload_form.file;
    let upload_file_handler: &dyn UploadFileHandler = file as &dyn UploadFileHandler;

    // state.file_service.validate_attached_file(*file).await?;

    // let file_upload_response = state
    //     .file_service
    //     .upload_file(upload_file_handler)
    //     .await?;

    // Ok(Json(file_upload_response))
    return Err(Error::PreconditionFailed);
}

#[get("/<id>")]
pub async fn detail(
    state: &State<ServiceContext>,
    id: &str,
) -> Result<Json<data::CompanyToReturn>> {
    return Err(Error::PreconditionFailed);
}

#[post("/create", format = "json", data = "<create_company_payload>")]
pub async fn create(
    state: &State<ServiceContext>,
    create_company_payload: Json<CreateCompanyPayload>,
) -> Result<Json<data::CompanyToReturn>> {
    return Err(Error::PreconditionFailed);
}

#[put("/edit", format = "json", data = "<edit_company_payload>")]
pub async fn edit(
    state: &State<ServiceContext>,
    edit_company_payload: Json<EditCompanyPayload>,
) -> Result<()> {
    return Err(Error::PreconditionFailed);
}

#[put("/add_signatory", format = "json", data = "<add_signatory_payload>")]
pub async fn add_signatory(
    state: &State<ServiceContext>,
    add_signatory_payload: Json<AddSignatoryPayload>,
) -> Result<()> {
    return Err(Error::PreconditionFailed);
}

#[put(
    "/remove_signatory",
    format = "json",
    data = "<remove_signatory_payload>"
)]
pub async fn remove_signatory(
    state: &State<ServiceContext>,
    remove_signatory_payload: Json<RemoveSignatoryPayload>,
) -> Result<()> {
    return Err(Error::PreconditionFailed);
}
