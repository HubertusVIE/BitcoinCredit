use crate::constants::{BILLS_FOLDER_PATH, BILLS_KEYS_FOLDER_PATH};
use crate::service::bill_service::BillKeys;
use crate::service::bill_service::BitcreditBill;
use borsh::BorshDeserialize;
use std::fs;
use std::path::PathBuf;

pub mod quotes;

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

pub fn bill_from_byte_array(bill: &[u8]) -> BitcreditBill {
    BitcreditBill::try_from_slice(bill).unwrap()
}

pub fn read_keys_from_bill_file(bill_name: &str) -> BillKeys {
    let input_path = get_path_for_bill_keys(bill_name);
    let blockchain_from_file = fs::read(input_path.clone()).expect("file not found");
    serde_json::from_slice(blockchain_from_file.as_slice()).unwrap()
}
