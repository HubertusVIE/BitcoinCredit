use crate::blockchain::{Chain, ChainToReturn, OperationCode};
use crate::constants::{BILLS_FOLDER_PATH, BILLS_KEYS_FOLDER_PATH};
use crate::external;
use crate::service::contact_service::IdentityPublicData;
use crate::util::file::is_not_hidden_or_directory;
use borsh::BorshDeserialize;
use borsh_derive::{BorshDeserialize, BorshSerialize};
use rocket::serde::{Deserialize, Serialize};
use rocket::FromForm;
use std::fs;
use std::path::PathBuf;

pub mod contacts;
pub mod identity;
pub mod quotes;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct BitcreditBillToReturn {
    pub name: String,
    pub to_payee: bool,
    pub bill_jurisdiction: String,
    pub timestamp_at_drawing: i64,
    pub drawee: IdentityPublicData,
    pub drawer: IdentityPublicData,
    pub payee: IdentityPublicData,
    pub endorsee: IdentityPublicData,
    pub place_of_drawing: String,
    pub currency_code: String,
    pub amount_numbers: u64,
    pub amounts_letters: String,
    pub maturity_date: String,
    pub date_of_issue: String,
    pub compounding_interest_rate: u64,
    pub type_of_interest_calculation: bool,
    pub place_of_payment: String,
    pub public_key: String,
    pub private_key: String,
    pub language: String,
    pub accepted: bool,
    pub endorsed: bool,
    pub requested_to_pay: bool,
    pub requested_to_accept: bool,
    pub paid: bool,
    pub waited_for_payment: bool,
    pub address_for_selling: String,
    pub amount_for_selling: u64,
    pub buyer: IdentityPublicData,
    pub seller: IdentityPublicData,
    pub link_for_buy: String,
    pub link_to_pay: String,
    pub pr_key_bill: String,
    pub number_of_confirmations: u64,
    pub pending: bool,
    pub address_to_pay: String,
    pub chain_of_blocks: ChainToReturn,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct BitcreditEbillQuote {
    pub bill_id: String,
    pub quote_id: String,
    pub amount: u64,
    pub mint_node_id: String,
    pub mint_url: String,
    pub accepted: bool,
    pub token: String,
}

impl BitcreditEbillQuote {
    pub fn new_empty() -> Self {
        Self {
            bill_id: "".to_string(),
            quote_id: "".to_string(),
            amount: 0,
            mint_node_id: "".to_string(),
            mint_url: "".to_string(),
            accepted: false,
            token: "".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct BitcreditBillForList {
    name: String,
    to_payee: bool,
    bill_jurisdiction: String,
    timestamp_at_drawing: i64,
    drawee: IdentityPublicData,
    drawer: IdentityPublicData,
    payee: IdentityPublicData,
    endorsee: IdentityPublicData,
    place_of_drawing: String,
    currency_code: String,
    amount_numbers: u64,
    amounts_letters: String,
    maturity_date: String,
    date_of_issue: String,
    compounding_interest_rate: u64,
    type_of_interest_calculation: bool,
    place_of_payment: String,
    public_key: String,
    private_key: String,
    language: String,
    chain_of_blocks: ChainToReturn,
}

#[derive(BorshSerialize, BorshDeserialize, FromForm, Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct BitcreditBill {
    pub name: String,
    pub to_payee: bool,
    pub bill_jurisdiction: String,
    pub timestamp_at_drawing: i64,
    // The party obliged to pay a Bill
    pub drawee: IdentityPublicData,
    // The party issuing a Bill
    pub drawer: IdentityPublicData,
    // The person to whom the Payee or an Endorsee endorses a bill
    pub payee: IdentityPublicData,
    pub endorsee: IdentityPublicData,
    pub place_of_drawing: String,
    pub currency_code: String,
    //TODO: f64
    pub amount_numbers: u64,
    pub amounts_letters: String,
    pub maturity_date: String,
    pub date_of_issue: String,
    pub compounding_interest_rate: u64,
    pub type_of_interest_calculation: bool,
    // Defaulting to the drawee’s id/ address.
    pub place_of_payment: String,
    pub public_key: String,
    pub private_key: String,
    pub language: String,
    pub files: Vec<BillFile>,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, FromForm, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct BillFile {
    pub name: String,
    pub hash: String,
}

#[cfg(test)]
impl BitcreditBill {
    pub fn new_empty() -> Self {
        Self {
            name: "".to_string(),
            to_payee: false,
            bill_jurisdiction: "".to_string(),
            timestamp_at_drawing: 0,
            drawee: IdentityPublicData::new_empty(),
            drawer: IdentityPublicData::new_empty(),
            payee: IdentityPublicData::new_empty(),
            endorsee: IdentityPublicData::new_empty(),
            place_of_drawing: "".to_string(),
            currency_code: "".to_string(),
            amount_numbers: 0,
            amounts_letters: "".to_string(),
            maturity_date: "".to_string(),
            date_of_issue: "".to_string(),
            compounding_interest_rate: 0,
            type_of_interest_calculation: false,
            place_of_payment: "".to_string(),
            public_key: "".to_string(),
            private_key: "".to_string(),
            language: "".to_string(),
            files: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BillKeys {
    pub private_key_pem: String,
    pub public_key_pem: String,
}

pub fn get_path_for_bill(bill_name: &str) -> PathBuf {
    let mut path = PathBuf::from(BILLS_FOLDER_PATH).join(bill_name);
    path.set_extension("json");
    path
}

pub fn get_path_for_bill_keys(key_name: &str) -> PathBuf {
    let mut path = PathBuf::from(BILLS_KEYS_FOLDER_PATH).join(key_name);
    path.set_extension("json");
    path
}

pub async fn get_bills() -> Vec<BitcreditBill> {
    let mut bills = Vec::new();
    let paths = fs::read_dir(BILLS_FOLDER_PATH).unwrap();
    for path in paths {
        let dir = path.unwrap();
        if is_not_hidden_or_directory(&dir) {
            let bill = read_bill_from_file(
                dir.path()
                    .file_stem()
                    .expect("File name error")
                    .to_str()
                    .expect("File name error"),
            )
            .await;
            bills.push(bill);
        }
    }
    bills
}

pub async fn get_bills_for_list() -> Vec<BitcreditBillToReturn> {
    let mut bills = Vec::new();
    let paths = fs::read_dir(BILLS_FOLDER_PATH).unwrap();
    for path in paths {
        let dir = path.unwrap();
        if is_not_hidden_or_directory(&dir) {
            let bill = read_bill_with_chain_from_file(
                dir.path()
                    .file_stem()
                    .expect("File name error")
                    .to_str()
                    .expect("File name error"),
            )
            .await;
            bills.push(bill);
        }
    }
    bills
}

async fn read_bill_with_chain_from_file(id: &str) -> BitcreditBillToReturn {
    let bill: BitcreditBill = read_bill_from_file(id).await;
    let chain = Chain::read_chain_from_file(&bill.name);
    let drawer = chain.get_drawer();
    let chain_to_return = ChainToReturn::new(chain.clone());
    let endorsed = chain.exist_block_with_operation_code(OperationCode::Endorse);
    let accepted = chain.exist_block_with_operation_code(OperationCode::Accept);
    let requested_to_pay = chain.exist_block_with_operation_code(OperationCode::RequestToPay);
    let requested_to_accept = chain.exist_block_with_operation_code(OperationCode::RequestToAccept);
    let address_to_pay = external::bitcoin::get_address_to_pay(bill.clone());
    let mut paid = false;
    if chain.exist_block_with_operation_code(OperationCode::RequestToPay) {
        let check_if_already_paid =
            external::bitcoin::check_if_paid(address_to_pay.clone(), bill.amount_numbers).await;
        paid = check_if_already_paid.0;
    }

    BitcreditBillToReturn {
        name: bill.name,
        to_payee: bill.to_payee,
        bill_jurisdiction: bill.bill_jurisdiction,
        timestamp_at_drawing: bill.timestamp_at_drawing,
        drawee: bill.drawee,
        drawer,
        payee: bill.payee,
        endorsee: bill.endorsee,
        place_of_drawing: bill.place_of_drawing,
        currency_code: bill.currency_code,
        amount_numbers: bill.amount_numbers,
        amounts_letters: bill.amounts_letters,
        maturity_date: bill.maturity_date,
        date_of_issue: bill.date_of_issue,
        compounding_interest_rate: bill.compounding_interest_rate,
        type_of_interest_calculation: bill.type_of_interest_calculation,
        place_of_payment: bill.place_of_payment,
        public_key: bill.public_key,
        private_key: bill.private_key,
        language: bill.language,
        accepted,
        endorsed,
        waited_for_payment: false,
        address_for_selling: "".to_string(),
        amount_for_selling: 0,
        buyer: IdentityPublicData::new_empty(),
        seller: IdentityPublicData::new_empty(),
        requested_to_pay,
        requested_to_accept,
        paid,
        link_to_pay: "".to_string(),
        link_for_buy: "".to_string(),
        pr_key_bill: "".to_string(),
        number_of_confirmations: 0,
        pending: false,
        address_to_pay,
        chain_of_blocks: chain_to_return,
    }
}

pub async fn read_bill_from_file(bill_name: &str) -> BitcreditBill {
    let chain = Chain::read_chain_from_file(bill_name);
    chain.get_last_version_bill().await
}

pub fn bill_from_byte_array(bill: &[u8]) -> BitcreditBill {
    BitcreditBill::try_from_slice(bill).unwrap()
}

pub fn read_keys_from_bill_file(bill_name: &str) -> BillKeys {
    let input_path = get_path_for_bill_keys(bill_name);
    let blockchain_from_file = fs::read(input_path.clone()).expect("file not found");
    serde_json::from_slice(blockchain_from_file.as_slice()).unwrap()
}
