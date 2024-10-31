use rocket::fs::TempFile;
use rocket::serde::{Deserialize, Serialize};
use rocket::FromForm;

#[derive(FromForm, Debug)]
pub struct BitcreditBillForm<'r> {
    pub bill_jurisdiction: String,
    pub place_of_drawing: String,
    pub currency_code: String,
    pub amount_numbers: u64,
    pub language: String,
    pub drawee_name: String,
    pub payee_name: String,
    pub place_of_payment: String,
    pub maturity_date: String,
    pub drawer_is_payee: bool,
    pub drawer_is_drawee: bool,
    pub files: Vec<TempFile<'r>>,
}

#[derive(FromForm, Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct EndorseBitcreditBillForm {
    pub endorsee: String,
    pub bill_name: String,
}

#[derive(FromForm, Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct MintBitcreditBillForm {
    pub mint_node: String,
    pub bill_name: String,
}

#[derive(FromForm, Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AcceptMintBitcreditBillForm {
    pub interest: u64,
    pub bill_name: String,
}

#[derive(FromForm, Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct RequestToMintBitcreditBillForm {
    pub mint_node: String,
    pub bill_name: String,
}

#[derive(FromForm, Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct SellBitcreditBillForm {
    pub buyer: String,
    pub bill_name: String,
    pub amount_numbers: u64,
    pub currency_code: String,
}

#[derive(FromForm, Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct RequestToAcceptBitcreditBillForm {
    pub bill_name: String,
}

#[derive(FromForm, Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct RequestToPayBitcreditBillForm {
    pub bill_name: String,
}

#[derive(FromForm, Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AcceptBitcreditBillForm {
    pub bill_name: String,
}

#[derive(FromForm, Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct IdentityForm {
    pub name: String,
    pub company: String,
    pub date_of_birth: String,
    pub city_of_birth: String,
    pub country_of_birth: String,
    pub email: String,
    pub postal_address: String,
}

#[derive(FromForm, Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct NewContactForm {
    pub name: String,
    pub node_id: String,
}

#[derive(FromForm, Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct EditContactForm {
    pub old_name: String,
    pub name: String,
}

#[derive(FromForm, Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct DeleteContactForm {
    pub name: String,
}
