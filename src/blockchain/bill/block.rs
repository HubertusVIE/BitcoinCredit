use super::super::{Error, Result};
use super::extract_after_phrase;
use super::BillOpCode;
use super::BillOpCode::{Accept, Endorse, Issue, Mint, RequestToAccept, RequestToPay, Sell};

use crate::blockchain::Block;
use crate::constants::ACCEPTED_BY;
use crate::constants::ENDORSED_BY;
use crate::constants::ENDORSED_TO;
use crate::constants::REQ_TO_ACCEPT_BY;
use crate::constants::REQ_TO_PAY_BY;
use crate::constants::SOLD_BY;
use crate::constants::SOLD_TO;
use crate::service::bill_service::BillKeys;
use crate::service::bill_service::BitcreditBill;
use crate::service::contact_service::IdentityPublicData;
use crate::util::BcrKeys;
use crate::util::{self, crypto};

use borsh::from_slice;
use borsh_derive::BorshSerialize;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BillBlock {
    pub id: u64,
    pub hash: String,
    pub timestamp: u64,
    pub data: String,
    pub public_key: String,
    pub previous_hash: String,
    pub signature: String,
    pub operation_code: BillOpCode,
}

#[derive(BorshSerialize)]
pub struct BillBlockDataToHash {
    id: u64,
    previous_hash: String,
    data: String,
    timestamp: u64,
    public_key: String,
    operation_code: BillOpCode,
}

impl Block for BillBlock {
    type OpCode = BillOpCode;
    type BlockDataToHash = BillBlockDataToHash;

    fn id(&self) -> u64 {
        self.id
    }

    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    fn op_code(&self) -> &Self::OpCode {
        &self.operation_code
    }

    fn hash(&self) -> &str {
        &self.hash
    }

    fn previous_hash(&self) -> &str {
        &self.previous_hash
    }

    fn data(&self) -> &str {
        &self.data
    }

    fn signature(&self) -> &str {
        &self.signature
    }

    fn public_key(&self) -> &str {
        &self.public_key
    }

    fn get_block_data_to_hash(&self) -> Self::BlockDataToHash {
        let data = BillBlockDataToHash {
            id: self.id(),
            previous_hash: self.previous_hash().to_owned(),
            data: self.data().to_owned(),
            timestamp: self.timestamp(),
            public_key: self.public_key().to_owned(),
            operation_code: self.op_code().to_owned(),
        };
        data
    }
}

impl BillBlock {
    /// Creates a new instance of the struct with the provided details, calculates the block hash,
    /// and generates a signature for the block.
    ///
    /// # Arguments
    ///
    /// - `id`: The unique identifier of the block (`u64`).
    /// - `previous_hash`: A `String` representing the hash of the previous block in the chain.
    /// - `data`: A `String` containing the data to be stored in the block.
    /// - `operation_code`: An `BillOpCode` indicating the operation type associated with the block.
    /// - `keys`: The identity keys
    /// - `timestamp`: An `u64` timestamp representing the time the block was created.
    ///
    /// # Returns
    ///
    /// A new instance of the struct populated with the provided data, a calculated block hash,
    /// and a signature.
    ///
    pub fn new(
        id: u64,
        previous_hash: String,
        data: String,
        operation_code: BillOpCode,
        keys: BcrKeys,
        timestamp: u64,
    ) -> Result<Self> {
        let hash = Self::calculate_hash(BillBlockDataToHash {
            id,
            previous_hash: previous_hash.clone(),
            data: data.clone(),
            timestamp,
            public_key: keys.get_public_key(),
            operation_code: operation_code.clone(),
        })?;
        let signature = crypto::signature(&hash, &keys.get_private_key_string())?;

        Ok(Self {
            id,
            hash,
            timestamp,
            previous_hash,
            signature,
            public_key: keys.get_public_key(),
            data,
            operation_code,
        })
    }

    /// Decrypts the block data using the bill's private key, returning a String
    pub fn get_decrypted_block_data(&self, bill_keys: &BillKeys) -> Result<String> {
        let decrypted_bytes = self.get_decrypted_block_bytes(bill_keys)?;
        let block_data_decrypted = String::from_utf8(decrypted_bytes)?;
        Ok(block_data_decrypted)
    }

    /// Decrypts the block data using the bill's private key, returning the raw bytes
    pub fn get_decrypted_block_bytes(&self, bill_keys: &BillKeys) -> Result<Vec<u8>> {
        let bytes = util::base58_decode(&self.data)?;
        let decrypted_bytes = util::crypto::decrypt_ecies(&bytes, &bill_keys.private_key)?;
        Ok(decrypted_bytes)
    }

    /// Extracts a list of unique node IDs involved in a block operation.
    ///
    /// # Parameters
    /// - `bill_keys`: The bill's keys
    ///
    /// # Returns
    /// A `Vec<String>` containing the unique peer IDs involved in the block. Peer IDs are included
    /// only if they are non-empty.
    ///
    pub fn get_nodes_from_block(&self, bill_keys: &BillKeys) -> Result<Vec<String>> {
        let mut nodes = HashSet::new();
        match self.operation_code {
            Issue => {
                let bill: BitcreditBill = from_slice(&self.get_decrypted_block_bytes(bill_keys)?)?;

                let drawer_name = &bill.drawer.node_id;
                if !drawer_name.is_empty() {
                    nodes.insert(drawer_name.to_owned());
                }

                let payee_name = &bill.payee.node_id;
                if !payee_name.is_empty() {
                    nodes.insert(payee_name.to_owned());
                }

                let drawee_name = &bill.drawee.node_id;
                if !drawee_name.is_empty() {
                    nodes.insert(drawee_name.to_owned());
                }
            }
            Endorse => {
                let block_data_decrypted = self.get_decrypted_block_data(bill_keys)?;

                let endorsee: IdentityPublicData = serde_json::from_slice(&util::base58_decode(
                    &extract_after_phrase(&block_data_decrypted, ENDORSED_TO).ok_or(
                        Error::InvalidBlockdata(String::from("Endorse: No endorsee found")),
                    )?,
                )?)?;
                let endorsee_node_id = endorsee.node_id;
                if !endorsee_node_id.is_empty() {
                    nodes.insert(endorsee_node_id);
                }

                let endorser: IdentityPublicData = serde_json::from_slice(&util::base58_decode(
                    &extract_after_phrase(&block_data_decrypted, ENDORSED_BY).ok_or(
                        Error::InvalidBlockdata(String::from("Endorse: No endorser found")),
                    )?,
                )?)?;
                let endorser_node_id = endorser.node_id;
                if !endorser_node_id.is_empty() {
                    nodes.insert(endorser_node_id);
                }
            }
            Mint => {
                let block_data_decrypted = self.get_decrypted_block_data(bill_keys)?;

                let mint: IdentityPublicData = serde_json::from_slice(&util::base58_decode(
                    &extract_after_phrase(&block_data_decrypted, ENDORSED_TO)
                        .ok_or(Error::InvalidBlockdata(String::from("Mint: No mint found")))?,
                )?)?;
                let mint_node_id = mint.node_id;
                if !mint_node_id.is_empty() {
                    nodes.insert(mint_node_id);
                }

                let minter: IdentityPublicData = serde_json::from_slice(&util::base58_decode(
                    &extract_after_phrase(&block_data_decrypted, ENDORSED_BY).ok_or(
                        Error::InvalidBlockdata(String::from("Mint: No minter found")),
                    )?,
                )?)?;
                let minter_node_id = minter.node_id;
                if !minter_node_id.is_empty() {
                    nodes.insert(minter_node_id);
                }
            }
            RequestToAccept => {
                let block_data_decrypted = self.get_decrypted_block_data(bill_keys)?;

                let requester: IdentityPublicData = serde_json::from_slice(&util::base58_decode(
                    &extract_after_phrase(&block_data_decrypted, REQ_TO_ACCEPT_BY).ok_or(
                        Error::InvalidBlockdata(String::from(
                            "Request to accept: No requester found",
                        )),
                    )?,
                )?)?;
                let requester_node_id = requester.node_id;
                if !requester_node_id.is_empty() {
                    nodes.insert(requester_node_id);
                }
            }
            Accept => {
                let block_data_decrypted = self.get_decrypted_block_data(bill_keys)?;

                let accepter: IdentityPublicData = serde_json::from_slice(&util::base58_decode(
                    &extract_after_phrase(&block_data_decrypted, ACCEPTED_BY).ok_or(
                        Error::InvalidBlockdata(String::from("Accept: No accepter found")),
                    )?,
                )?)?;
                let accepter_node_id = accepter.node_id;
                if !accepter_node_id.is_empty() {
                    nodes.insert(accepter_node_id);
                }
            }
            RequestToPay => {
                let block_data_decrypted = self.get_decrypted_block_data(bill_keys)?;

                let requester: IdentityPublicData = serde_json::from_slice(&util::base58_decode(
                    &extract_after_phrase(&block_data_decrypted, REQ_TO_PAY_BY).ok_or(
                        Error::InvalidBlockdata(String::from("Request to Pay: No requester found")),
                    )?,
                )?)?;
                let requester_node_id = requester.node_id;
                if !requester_node_id.is_empty() {
                    nodes.insert(requester_node_id);
                }
            }
            Sell => {
                let block_data_decrypted = self.get_decrypted_block_data(bill_keys)?;

                let buyer: IdentityPublicData = serde_json::from_slice(&util::base58_decode(
                    &extract_after_phrase(&block_data_decrypted, SOLD_TO).ok_or(
                        Error::InvalidBlockdata(String::from("Sell: No buyer found")),
                    )?,
                )?)?;
                let buyer_node_id = buyer.node_id;
                if !buyer_node_id.is_empty() {
                    nodes.insert(buyer_node_id);
                }

                let seller: IdentityPublicData = serde_json::from_slice(&util::base58_decode(
                    &extract_after_phrase(&block_data_decrypted, SOLD_BY).ok_or(
                        Error::InvalidBlockdata(String::from("Sell: No seller found")),
                    )?,
                )?)?;
                let seller_node_id = seller.node_id;
                if !seller_node_id.is_empty() {
                    nodes.insert(seller_node_id);
                }
            }
        }
        Ok(nodes.into_iter().collect())
    }

    /// Generates a human-readable history label for a bill based on the operation code.
    ///
    /// # Parameters
    /// - `bill_keys`: The bill's keys
    ///
    /// # Returns
    /// A `String` representing the history label for the given bill.
    ///
    pub fn get_history_label(&self, bill_keys: &BillKeys) -> Result<String> {
        match self.operation_code {
            Issue => {
                let time_of_issue = util::date::seconds(self.timestamp);
                let bill: BitcreditBill = from_slice(&self.get_decrypted_block_bytes(bill_keys)?)?;
                if !bill.drawer.name.is_empty() {
                    Ok(format!(
                        "Bill issued by {} at {} in {}",
                        bill.drawer.name, time_of_issue, bill.place_of_drawing
                    ))
                } else if bill.to_payee {
                    Ok(format!(
                        "Bill issued by {} at {} in {}",
                        bill.payee.name, time_of_issue, bill.place_of_drawing
                    ))
                } else {
                    Ok(format!(
                        "Bill issued by {} at {} in {}",
                        bill.drawee.name, time_of_issue, bill.place_of_drawing
                    ))
                }
            }
            Endorse => {
                let block_data_decrypted = self.get_decrypted_block_data(bill_keys)?;

                let endorser: IdentityPublicData = serde_json::from_slice(&util::base58_decode(
                    &extract_after_phrase(&block_data_decrypted, ENDORSED_BY).ok_or(
                        Error::InvalidBlockdata(String::from("Endorse: No endorser found")),
                    )?,
                )?)?;

                Ok(format!("{}, {}", endorser.name, endorser.postal_address))
            }
            Mint => {
                let block_data_decrypted = self.get_decrypted_block_data(bill_keys)?;

                let minter: IdentityPublicData = serde_json::from_slice(&util::base58_decode(
                    &extract_after_phrase(&block_data_decrypted, ENDORSED_BY).ok_or(
                        Error::InvalidBlockdata(String::from("Mint: No minter found")),
                    )?,
                )?)?;

                Ok(format!("{}, {}", minter.name, minter.postal_address))
            }
            RequestToAccept => {
                let time_of_request_to_accept = util::date::seconds(self.timestamp);
                let block_data_decrypted = self.get_decrypted_block_data(bill_keys)?;

                let requester: IdentityPublicData = serde_json::from_slice(&util::base58_decode(
                    &extract_after_phrase(&block_data_decrypted, REQ_TO_ACCEPT_BY).ok_or(
                        Error::InvalidBlockdata(String::from(
                            "Request to accept: No requester found",
                        )),
                    )?,
                )?)?;

                Ok(format!(
                    "Bill requested to accept by {} at {} in {}",
                    requester.name, time_of_request_to_accept, requester.postal_address
                ))
            }
            Accept => {
                let time_of_accept = util::date::seconds(self.timestamp);
                let block_data_decrypted = self.get_decrypted_block_data(bill_keys)?;

                let accepter: IdentityPublicData = serde_json::from_slice(&util::base58_decode(
                    &extract_after_phrase(&block_data_decrypted, ACCEPTED_BY).ok_or(
                        Error::InvalidBlockdata(String::from("Accept: No accepter found")),
                    )?,
                )?)?;

                Ok(format!(
                    "Bill accepted by {} at {} in {}",
                    accepter.name, time_of_accept, accepter.postal_address
                ))
            }
            RequestToPay => {
                let time_of_request_to_pay = util::date::seconds(self.timestamp);
                let block_data_decrypted = self.get_decrypted_block_data(bill_keys)?;

                let requester: IdentityPublicData = serde_json::from_slice(&util::base58_decode(
                    &extract_after_phrase(&block_data_decrypted, REQ_TO_PAY_BY).ok_or(
                        Error::InvalidBlockdata(String::from("Request to pay: No requester found")),
                    )?,
                )?)?;

                Ok(format!(
                    "Bill requested to pay by {} at {} in {}",
                    requester.name, time_of_request_to_pay, requester.postal_address
                ))
            }
            Sell => {
                let block_data_decrypted = self.get_decrypted_block_data(bill_keys)?;

                let seller: IdentityPublicData = serde_json::from_slice(&util::base58_decode(
                    &extract_after_phrase(&block_data_decrypted, SOLD_BY).ok_or(
                        Error::InvalidBlockdata(String::from("Sell: No seller found")),
                    )?,
                )?)?;

                Ok(format!("{}, {}", seller.name, seller.postal_address))
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tests::test::{get_bill_keys, TEST_PUB_KEY_SECP};
    use borsh::to_vec;

    #[test]
    fn signature_can_be_verified() {
        let block = BillBlock::new(
            1,
            String::from("prevhash"),
            String::from("some_data"),
            BillOpCode::Issue,
            BcrKeys::new(),
            1731593928,
        )
        .unwrap();
        assert!(block.verify());
    }

    #[test]
    fn get_nodes_from_block_issue() {
        let mut bill = BitcreditBill::new_empty();
        let mut drawer = IdentityPublicData::new_empty();
        let node_id = BcrKeys::new().get_public_key();
        let mut payer = IdentityPublicData::new_empty();
        let payer_node_id = BcrKeys::new().get_public_key();
        payer.node_id = payer_node_id.clone();
        drawer.node_id = node_id.clone();
        bill.drawer = drawer.clone();
        bill.payee = drawer.clone();
        bill.drawee = payer;

        let hashed_bill = util::base58_encode(
            &util::crypto::encrypt_ecies(&to_vec(&bill).unwrap(), TEST_PUB_KEY_SECP).unwrap(),
        );

        let block = BillBlock::new(
            1,
            String::from("prevhash"),
            hashed_bill,
            BillOpCode::Issue,
            BcrKeys::new(),
            1731593928,
        )
        .unwrap();
        let res = block.get_nodes_from_block(&get_bill_keys());
        assert!(res.is_ok());
        assert_eq!(res.as_ref().unwrap().len(), 2);
        assert!(res.as_ref().unwrap().contains(&node_id));
        assert!(res.as_ref().unwrap().contains(&payer_node_id));
    }

    #[test]
    fn get_history_label_issue() {
        let mut bill = BitcreditBill::new_empty();
        bill.place_of_drawing = "Vienna".to_string();
        let mut drawer = IdentityPublicData::new_empty();
        drawer.name = "bill".to_string();
        bill.drawer = drawer.clone();

        let hashed_bill = util::base58_encode(
            &util::crypto::encrypt_ecies(&to_vec(&bill).unwrap(), TEST_PUB_KEY_SECP).unwrap(),
        );

        let block = BillBlock::new(
            1,
            String::from("prevhash"),
            hashed_bill,
            BillOpCode::Issue,
            BcrKeys::new(),
            1731593928,
        )
        .unwrap();
        let res = block.get_history_label(&get_bill_keys());
        assert!(res.is_ok());
        assert_eq!(
            res.as_ref().unwrap(),
            "Bill issued by bill at 2024-11-14 14:18:48 UTC in Vienna"
        );
    }

    #[test]
    fn get_nodes_from_block_endorse() {
        let mut endorsee = IdentityPublicData::new_empty();
        let node_id = BcrKeys::new().get_public_key();
        endorsee.node_id = node_id.clone();
        let mut endorser = IdentityPublicData::new_empty();
        let endorser_node_id = BcrKeys::new().get_public_key();
        endorser.node_id = endorser_node_id.clone();
        let hashed_endorsee = util::base58_encode(&serde_json::to_vec(&endorsee).unwrap());
        let hashed_endorser = util::base58_encode(&serde_json::to_vec(&endorser).unwrap());

        let data = format!(
            "{}{}{}{}",
            ENDORSED_TO, &hashed_endorsee, ENDORSED_BY, &hashed_endorser
        );

        let block = BillBlock::new(
            1,
            String::from("prevhash"),
            util::base58_encode(
                &util::crypto::encrypt_ecies(data.as_bytes(), TEST_PUB_KEY_SECP).unwrap(),
            ),
            BillOpCode::Endorse,
            BcrKeys::new(),
            1731593928,
        )
        .unwrap();
        let res = block.get_nodes_from_block(&get_bill_keys());
        assert!(res.is_ok());
        assert_eq!(res.as_ref().unwrap().len(), 2);
        assert!(res.as_ref().unwrap().contains(&node_id));
        assert!(res.as_ref().unwrap().contains(&endorser_node_id));
    }

    #[test]
    fn get_history_label_endorse() {
        let endorsee = IdentityPublicData::new_empty();
        let mut endorser = IdentityPublicData::new_empty();
        endorser.name = "bill".to_string();
        endorser.postal_address = "some street 1".to_string();
        let hashed_endorsee = util::base58_encode(&serde_json::to_vec(&endorsee).unwrap());
        let hashed_endorser = util::base58_encode(&serde_json::to_vec(&endorser).unwrap());

        let data = format!(
            "{}{}{}{}",
            ENDORSED_TO, &hashed_endorsee, ENDORSED_BY, &hashed_endorser
        );

        let block = BillBlock::new(
            1,
            String::from("prevhash"),
            util::base58_encode(
                &util::crypto::encrypt_ecies(data.as_bytes(), TEST_PUB_KEY_SECP).unwrap(),
            ),
            BillOpCode::Endorse,
            BcrKeys::new(),
            1731593928,
        )
        .unwrap();
        let res = block.get_history_label(&get_bill_keys());
        assert!(res.is_ok());
        assert_eq!(res.as_ref().unwrap(), "bill, some street 1");
    }

    #[test]
    fn get_nodes_from_block_mint() {
        let mut mint = IdentityPublicData::new_empty();
        let node_id = BcrKeys::new().get_public_key();
        mint.node_id = node_id.clone();
        let mut minter = IdentityPublicData::new_empty();
        let minter_node_id = BcrKeys::new().get_public_key();
        minter.node_id = minter_node_id.clone();
        let hashed_mint = util::base58_encode(&serde_json::to_vec(&mint).unwrap());
        let hashed_minter = util::base58_encode(&serde_json::to_vec(&minter).unwrap());

        let data = format!(
            "{}{}{}{}",
            ENDORSED_TO, &hashed_mint, ENDORSED_BY, &hashed_minter
        );

        let block = BillBlock::new(
            1,
            String::from("prevhash"),
            util::base58_encode(
                &util::crypto::encrypt_ecies(data.as_bytes(), TEST_PUB_KEY_SECP).unwrap(),
            ),
            BillOpCode::Mint,
            BcrKeys::new(),
            1731593928,
        )
        .unwrap();
        let res = block.get_nodes_from_block(&get_bill_keys());
        assert!(res.is_ok());
        assert_eq!(res.as_ref().unwrap().len(), 2);
        assert!(res.as_ref().unwrap().contains(&node_id));
        assert!(res.as_ref().unwrap().contains(&minter_node_id));
    }

    #[test]
    fn get_history_label_mint() {
        let mint = IdentityPublicData::new_empty();
        let mut minter = IdentityPublicData::new_empty();
        minter.name = "bill".to_string();
        minter.postal_address = "some street 1".to_string();
        let hashed_endorsee = util::base58_encode(&serde_json::to_vec(&mint).unwrap());
        let hashed_endorser = util::base58_encode(&serde_json::to_vec(&minter).unwrap());

        let data = format!(
            "{}{}{}{}",
            ENDORSED_TO, &hashed_endorsee, ENDORSED_BY, &hashed_endorser
        );

        let block = BillBlock::new(
            1,
            String::from("prevhash"),
            util::base58_encode(
                &util::crypto::encrypt_ecies(data.as_bytes(), TEST_PUB_KEY_SECP).unwrap(),
            ),
            BillOpCode::Mint,
            BcrKeys::new(),
            1731593928,
        )
        .unwrap();
        let res = block.get_history_label(&get_bill_keys());
        assert!(res.is_ok());
        assert_eq!(res.as_ref().unwrap(), "bill, some street 1");
    }

    #[test]
    fn get_nodes_from_block_req_to_accept() {
        let mut requester = IdentityPublicData::new_empty();
        let node_id = BcrKeys::new().get_public_key();
        requester.node_id = node_id.clone();
        let hashed_requester = util::base58_encode(&serde_json::to_vec(&requester).unwrap());

        let data = format!("{}{}", REQ_TO_ACCEPT_BY, &hashed_requester);

        let block = BillBlock::new(
            1,
            String::from("prevhash"),
            util::base58_encode(
                &util::crypto::encrypt_ecies(data.as_bytes(), TEST_PUB_KEY_SECP).unwrap(),
            ),
            BillOpCode::RequestToAccept,
            BcrKeys::new(),
            1731593928,
        )
        .unwrap();
        let res = block.get_nodes_from_block(&get_bill_keys());
        assert!(res.is_ok());
        assert_eq!(res.as_ref().unwrap().len(), 1);
        assert!(res.as_ref().unwrap().contains(&node_id));
    }

    #[test]
    fn get_history_label_req_to_accept() {
        let mut requester = IdentityPublicData::new_empty();
        requester.name = "bill".to_string();
        requester.postal_address = "some street 1".to_string();
        let hashed_requester = util::base58_encode(&serde_json::to_vec(&requester).unwrap());

        let data = format!("{}{}", REQ_TO_ACCEPT_BY, &hashed_requester);

        let block = BillBlock::new(
            1,
            String::from("prevhash"),
            util::base58_encode(
                &util::crypto::encrypt_ecies(data.as_bytes(), TEST_PUB_KEY_SECP).unwrap(),
            ),
            BillOpCode::RequestToAccept,
            BcrKeys::new(),
            1731593928,
        )
        .unwrap();
        let res = block.get_history_label(&get_bill_keys());
        assert!(res.is_ok());
        assert_eq!(
            res.as_ref().unwrap(),
            "Bill requested to accept by bill at 2024-11-14 14:18:48 UTC in some street 1"
        );
    }

    #[test]
    fn get_nodes_from_block_accept() {
        let mut accepter = IdentityPublicData::new_empty();
        let node_id = BcrKeys::new().get_public_key();
        accepter.node_id = node_id.clone();
        let hashed_accepter = util::base58_encode(&serde_json::to_vec(&accepter).unwrap());

        let data = format!("{}{}", ACCEPTED_BY, &hashed_accepter);

        let block = BillBlock::new(
            1,
            String::from("prevhash"),
            util::base58_encode(
                &util::crypto::encrypt_ecies(data.as_bytes(), TEST_PUB_KEY_SECP).unwrap(),
            ),
            BillOpCode::Accept,
            BcrKeys::new(),
            1731593928,
        )
        .unwrap();
        let res = block.get_nodes_from_block(&get_bill_keys());
        assert!(res.is_ok());
        assert_eq!(res.as_ref().unwrap().len(), 1);
        assert!(res.as_ref().unwrap().contains(&node_id));
    }

    #[test]
    fn get_history_label_accept() {
        let mut accepter = IdentityPublicData::new_empty();
        accepter.name = "bill".to_string();
        accepter.postal_address = "some street 1".to_string();
        let hashed_accepter = util::base58_encode(&serde_json::to_vec(&accepter).unwrap());

        let data = format!("{}{}", ACCEPTED_BY, &hashed_accepter);

        let block = BillBlock::new(
            1,
            String::from("prevhash"),
            util::base58_encode(
                &util::crypto::encrypt_ecies(data.as_bytes(), TEST_PUB_KEY_SECP).unwrap(),
            ),
            BillOpCode::Accept,
            BcrKeys::new(),
            1731593928,
        )
        .unwrap();
        let res = block.get_history_label(&get_bill_keys());
        assert!(res.is_ok());
        assert_eq!(
            res.as_ref().unwrap(),
            "Bill accepted by bill at 2024-11-14 14:18:48 UTC in some street 1"
        );
    }

    #[test]
    fn get_nodes_from_block_accept_fails_for_invalid_data() {
        let mut accepter = IdentityPublicData::new_empty();
        let node_id = BcrKeys::new().get_public_key();
        accepter.node_id = node_id.clone();
        let hashed_accepter = util::base58_encode(&serde_json::to_vec(&accepter).unwrap());

        let data = format!("{}{}", ACCEPTED_BY, &hashed_accepter);

        let block = BillBlock::new(
            1,
            String::from("prevhash"),
            // not encrypted
            util::base58_encode(data.as_bytes()),
            BillOpCode::Accept,
            BcrKeys::new(),
            1731593928,
        )
        .unwrap();
        let res = block.get_nodes_from_block(&get_bill_keys());
        assert!(res.is_err());
    }

    #[test]
    fn get_nodes_from_block_accept_fails_for_invalid_block() {
        let block = BillBlock::new(
            1,
            String::from("prevhash"),
            util::base58_encode(
                &util::crypto::encrypt_ecies("some data".to_string().as_bytes(), TEST_PUB_KEY_SECP)
                    .unwrap(),
            ),
            BillOpCode::Accept,
            BcrKeys::new(),
            1731593928,
        )
        .unwrap();
        let res = block.get_nodes_from_block(&get_bill_keys());
        assert!(res.is_err());
    }

    #[test]
    fn get_nodes_from_block_req_to_pay() {
        let mut requester = IdentityPublicData::new_empty();
        let node_id = BcrKeys::new().get_public_key();
        requester.node_id = node_id.clone();
        let hashed_requester = util::base58_encode(&serde_json::to_vec(&requester).unwrap());

        let data = format!("{}{}", REQ_TO_PAY_BY, &hashed_requester);

        let block = BillBlock::new(
            1,
            String::from("prevhash"),
            util::base58_encode(
                &util::crypto::encrypt_ecies(data.as_bytes(), TEST_PUB_KEY_SECP).unwrap(),
            ),
            BillOpCode::RequestToPay,
            BcrKeys::new(),
            1731593928,
        )
        .unwrap();
        let res = block.get_nodes_from_block(&get_bill_keys());
        assert!(res.is_ok());
        assert_eq!(res.as_ref().unwrap().len(), 1);
        assert!(res.as_ref().unwrap().contains(&node_id));
    }

    #[test]
    fn get_history_label_req_to_pay() {
        let mut requester = IdentityPublicData::new_empty();
        requester.name = "bill".to_string();
        requester.postal_address = "some street 1".to_string();
        let hashed_requester = util::base58_encode(&serde_json::to_vec(&requester).unwrap());

        let data = format!("{}{}", REQ_TO_PAY_BY, &hashed_requester);

        let block = BillBlock::new(
            1,
            String::from("prevhash"),
            util::base58_encode(
                &util::crypto::encrypt_ecies(data.as_bytes(), TEST_PUB_KEY_SECP).unwrap(),
            ),
            BillOpCode::RequestToPay,
            BcrKeys::new(),
            1731593928,
        )
        .unwrap();
        let res = block.get_history_label(&get_bill_keys());
        assert!(res.is_ok());
        assert_eq!(
            res.as_ref().unwrap(),
            "Bill requested to pay by bill at 2024-11-14 14:18:48 UTC in some street 1"
        );
    }

    #[test]
    fn get_nodes_from_block_sell() {
        let mut buyer = IdentityPublicData::new_empty();
        let node_id = BcrKeys::new().get_public_key();
        buyer.node_id = node_id.clone();
        let mut seller = IdentityPublicData::new_empty();
        let endorser_node_id = BcrKeys::new().get_public_key();
        seller.node_id = endorser_node_id.clone();
        let hashed_buyer = util::base58_encode(&serde_json::to_vec(&buyer).unwrap());
        let hashed_seller = util::base58_encode(&serde_json::to_vec(&seller).unwrap());

        let data = format!("{}{}{}{}", SOLD_TO, &hashed_buyer, SOLD_BY, &hashed_seller);

        let block = BillBlock::new(
            1,
            String::from("prevhash"),
            util::base58_encode(
                &util::crypto::encrypt_ecies(data.as_bytes(), TEST_PUB_KEY_SECP).unwrap(),
            ),
            BillOpCode::Sell,
            BcrKeys::new(),
            1731593928,
        )
        .unwrap();
        let res = block.get_nodes_from_block(&get_bill_keys());
        assert!(res.is_ok());
        assert_eq!(res.as_ref().unwrap().len(), 2);
        assert!(res.as_ref().unwrap().contains(&node_id));
        assert!(res.as_ref().unwrap().contains(&endorser_node_id));
    }

    #[test]
    fn get_history_label_sell() {
        let mut seller = IdentityPublicData::new_empty();
        seller.name = "bill".to_string();
        seller.postal_address = "some street 1".to_string();
        let hashed_seller = util::base58_encode(&serde_json::to_vec(&seller).unwrap());

        let data = format!("{}{}", SOLD_BY, &hashed_seller);

        let block = BillBlock::new(
            1,
            String::from("prevhash"),
            util::base58_encode(
                &util::crypto::encrypt_ecies(data.as_bytes(), TEST_PUB_KEY_SECP).unwrap(),
            ),
            BillOpCode::Sell,
            BcrKeys::new(),
            1731593928,
        )
        .unwrap();
        let res = block.get_history_label(&get_bill_keys());
        assert!(res.is_ok());
        assert_eq!(res.as_ref().unwrap(), "bill, some street 1");
    }
}
