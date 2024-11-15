pub mod file;
pub mod numbers_to_words;
pub mod rsa;
use crate::{bill::BitcreditBill, constants::USEDNET, service::identity_service::Identity};
use bitcoin::secp256k1::Scalar;
use openssl::sha::sha256;
use std::str::FromStr;
use uuid::Uuid;

#[cfg(not(test))]
pub fn get_uuid_v4() -> Uuid {
    Uuid::new_v4()
}

#[cfg(test)]
pub fn get_uuid_v4() -> Uuid {
    use uuid::uuid;
    uuid!("00000000-0000-0000-0000-000000000000")
}

pub fn sha256_hash(bytes: &[u8]) -> String {
    hex::encode(sha256(bytes))
}

pub fn get_current_payee_private_key(identity: Identity, bill: BitcreditBill) -> String {
    let private_key_bill = bitcoin::PrivateKey::from_str(&bill.private_key).unwrap();

    let private_key_bill_holder =
        bitcoin::PrivateKey::from_str(&identity.bitcoin_private_key).unwrap();

    let privat_key_bill = private_key_bill
        .inner
        .add_tweak(&Scalar::from(private_key_bill_holder.inner))
        .unwrap();

    bitcoin::PrivateKey::new(privat_key_bill, USEDNET).to_string()
}
