use super::super::{Error, Result};
use super::block::BillBlock;
use super::extract_after_phrase;
use super::BillOpCode;
use super::BillOpCode::{Endorse, Mint, Sell};
use super::PaymentInfo;
use super::WaitingForPayment;
use crate::blockchain::Blockchain;
use crate::constants::ENDORSED_TO;
use crate::constants::SOLD_BY;
use crate::constants::SOLD_TO;
use crate::constants::{AMOUNT, SIGNED_BY};
use crate::service::bill_service::BillKeys;
use crate::service::bill_service::BitcreditBill;
use crate::service::contact_service::IdentityPublicData;
use crate::util::{rsa, BcrKeys};
use borsh::{from_slice, to_vec};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BillBlockchain {
    blocks: Vec<BillBlock>,
}

impl Blockchain for BillBlockchain {
    type Block = BillBlock;

    fn blocks(&self) -> &Vec<Self::Block> {
        &self.blocks
    }

    fn blocks_mut(&mut self) -> &mut Vec<Self::Block> {
        &mut self.blocks
    }
}

impl BillBlockchain {
    /// Creates a new blockchain for the given bill, encrypting the metadata using the bill's public
    /// key
    pub fn new(
        bill: &BitcreditBill,
        drawer: IdentityPublicData,
        drawer_key_pair: BcrKeys,
        bill_public_key_pem: String,
        timestamp: i64,
    ) -> Result<Self> {
        let drawer_bytes = serde_json::to_vec(&drawer)?;
        let data_for_new_block = format!("{}{}", SIGNED_BY, hex::encode(drawer_bytes));

        let genesis_hash: String = hex::encode(data_for_new_block.as_bytes());

        let encrypted_and_hashed_bill_data = hex::encode(rsa::encrypt_bytes_with_public_key(
            &to_vec(bill)?,
            &bill_public_key_pem,
        )?);

        let first_block = BillBlock::new(
            1,
            genesis_hash,
            encrypted_and_hashed_bill_data,
            BillOpCode::Issue,
            drawer_key_pair,
            timestamp,
        )?;

        let chain = Self {
            blocks: vec![first_block],
        };
        Ok(chain)
    }

    /// Transforms the whole chain to pretty-printed JSON
    pub fn to_pretty_printed_json(&self) -> Result<String> {
        let res = serde_json::to_string_pretty(&self)?;
        Ok(res)
    }

    /// Checks if the the chain has Endorse, Mint, or Sell blocks in it
    pub fn has_been_endorsed_sold_or_minted(&self) -> bool {
        self.blocks.iter().any(|block| {
            matches!(
                block.operation_code,
                BillOpCode::Mint | BillOpCode::Sell | BillOpCode::Endorse
            )
        })
    }

    /// Checks if the the chain has Endorse, or Sell blocks in it
    pub fn has_been_endorsed_or_sold(&self) -> bool {
        self.blocks
            .iter()
            .any(|block| matches!(block.operation_code, BillOpCode::Sell | BillOpCode::Endorse))
    }

    /// Retrieves the last version of the Bitcredit bill by decrypting and processing the relevant blocks.
    ///
    /// # Arguments
    /// * `bill_keys` - The keys for the bill.
    ///
    /// # Returns
    /// A `BitcreditBill` object containing the most recent version of the bill, including the payee, endorsee,
    /// and other associated information.
    ///
    pub fn get_last_version_bill(&self, bill_keys: &BillKeys) -> Result<BitcreditBill> {
        let first_block = self.get_first_block();
        let decrypted_bytes = first_block.get_decrypted_block_bytes(bill_keys)?;
        let bill_first_version: BitcreditBill = from_slice(&decrypted_bytes)?;

        let mut last_endorsee = IdentityPublicData {
            peer_id: "".to_string(),
            name: "".to_string(),
            company: "".to_string(),
            bitcoin_public_key: "".to_string(),
            postal_address: "".to_string(),
            email: "".to_string(),
            rsa_public_key_pem: "".to_string(),
            nostr_npub: None,
            nostr_relay: None,
        };

        if self.blocks.len() > 1 && self.has_been_endorsed_sold_or_minted() {
            let last_version_block_endorse = self.get_last_version_block_with_op_code(Endorse);
            let last_version_block_mint = self.get_last_version_block_with_op_code(Mint);
            let last_version_block_sell = self.get_last_version_block_with_op_code(Sell);
            let last_block = self.get_latest_block();

            // TODO: check if the last sell block is paid
            // in the future, this will come from the database, filled by a job that runs regularly
            // and checks for the paid status
            let last_sell_block_is_paid = true;

            if (last_version_block_endorse.id < last_version_block_sell.id)
                && (last_version_block_mint.id < last_version_block_sell.id)
                && ((last_block.id > last_version_block_sell.id) || last_sell_block_is_paid)
            {
                let block_data_decrypted =
                    last_version_block_sell.get_decrypted_block_data(bill_keys)?;
                let buyer: IdentityPublicData = serde_json::from_slice(&hex::decode(
                    &extract_after_phrase(&block_data_decrypted, SOLD_TO).ok_or(
                        Error::InvalidBlockdata(String::from("Sell: No buyer found")),
                    )?,
                )?)?;

                last_endorsee = buyer;
            } else if self.block_with_operation_code_exists(Endorse.clone())
                && (last_version_block_endorse.id > last_version_block_mint.id)
            {
                let block_data_decrypted =
                    last_version_block_endorse.get_decrypted_block_data(bill_keys)?;
                let endorsee: IdentityPublicData = serde_json::from_slice(&hex::decode(
                    &extract_after_phrase(&block_data_decrypted, ENDORSED_TO).ok_or(
                        Error::InvalidBlockdata(String::from("Endorse: No endorsee found")),
                    )?,
                )?)?;

                last_endorsee = endorsee;
            } else if self.block_with_operation_code_exists(Mint.clone())
                && (last_version_block_mint.id > last_version_block_endorse.id)
            {
                let block_data_decrypted =
                    last_version_block_mint.get_decrypted_block_data(bill_keys)?;
                let mint: IdentityPublicData = serde_json::from_slice(&hex::decode(
                    &extract_after_phrase(&block_data_decrypted, ENDORSED_TO)
                        .ok_or(Error::InvalidBlockdata(String::from("Mint: No mint found")))?,
                )?)?;

                last_endorsee = mint;
            }
        }

        let mut payee = bill_first_version.payee;

        if !last_endorsee.peer_id.is_empty() {
            payee = last_endorsee.clone();
        }

        Ok(BitcreditBill {
            name: bill_first_version.name,
            to_payee: bill_first_version.to_payee,
            bill_jurisdiction: bill_first_version.bill_jurisdiction,
            timestamp_at_drawing: bill_first_version.timestamp_at_drawing,
            drawee: bill_first_version.drawee,
            drawer: bill_first_version.drawer,
            payee: payee.clone(),
            endorsee: last_endorsee.clone(),
            place_of_drawing: bill_first_version.place_of_drawing,
            currency_code: bill_first_version.currency_code,
            amount_numbers: bill_first_version.amount_numbers,
            amounts_letters: bill_first_version.amounts_letters,
            maturity_date: bill_first_version.maturity_date,
            date_of_issue: bill_first_version.date_of_issue,
            compounding_interest_rate: bill_first_version.compounding_interest_rate,
            type_of_interest_calculation: bill_first_version.type_of_interest_calculation,
            place_of_payment: bill_first_version.place_of_payment,
            public_key: bill_first_version.public_key,
            private_key: bill_first_version.private_key,
            language: bill_first_version.language,
            files: bill_first_version.files,
        })
    }

    /// Checks if the last block is a sell block, if it's deadline is still active and if so,
    /// returns the buyer, seller and amount
    pub fn is_last_sell_block_waiting_for_payment(
        &self,
        bill_keys: &BillKeys,
        current_timestamp: i64,
    ) -> Result<WaitingForPayment> {
        let last_block = self.get_latest_block();
        let last_version_block_sell = self.get_last_version_block_with_op_code(Sell);
        // we only wait for payment, if the last block is a Sell block
        if self.block_with_operation_code_exists(Sell.clone())
            && last_block.id == last_version_block_sell.id
        {
            // if the deadline is up, we're not waiting for payment anymore
            if self.check_if_payment_deadline_has_passed(current_timestamp) {
                return Ok(WaitingForPayment::No);
            }

            let block_data_decrypted =
                last_version_block_sell.get_decrypted_block_data(bill_keys)?;

            let buyer: IdentityPublicData = serde_json::from_slice(&hex::decode(
                &extract_after_phrase(&block_data_decrypted, SOLD_TO).ok_or(
                    Error::InvalidBlockdata(String::from("Sell: No buyer found")),
                )?,
            )?)?;
            let seller: IdentityPublicData = serde_json::from_slice(&hex::decode(
                &extract_after_phrase(&block_data_decrypted, SOLD_BY).ok_or(
                    Error::InvalidBlockdata(String::from("Sell: No seller found")),
                )?,
            )?)?;

            let amount: u64 = extract_after_phrase(&block_data_decrypted, AMOUNT)
                .ok_or(Error::InvalidBlockdata(String::from(
                    "Sell: No amount found",
                )))?
                .parse()
                .map_err(|_| {
                    Error::InvalidBlockdata(String::from("Sell: Amount was no valid number"))
                })?;

            Ok(WaitingForPayment::Yes(Box::new(PaymentInfo {
                buyer,
                seller,
                amount,
            })))
        } else {
            Ok(WaitingForPayment::No)
        }
    }

    /// This function checks if the payment deadline associated with the most recent sell block
    /// has passed.
    /// # Returns
    ///
    /// - `true` if the payment deadline for the last sell block has passed.
    /// - `false` if no sell block exists or the deadline has not passed.
    ///
    fn check_if_payment_deadline_has_passed(&self, current_timestamp: i64) -> bool {
        if self.block_with_operation_code_exists(Sell) {
            let last_version_block_sell = self.get_last_version_block_with_op_code(Sell);
            let timestamp = last_version_block_sell.timestamp;

            let period: i64 = (86400 * 2) as i64; // 2 days deadline
            let difference = current_timestamp - timestamp;
            difference > period
        } else {
            false
        }
    }

    /// This function extracts the first block's data, decrypts it using the private key
    /// associated with the bill, and then deserializes the decrypted data into a `BitcreditBill`
    /// object.
    ///
    /// # Arguments
    /// * `bill_keys` - The keys for the bill.
    ///
    /// # Returns
    ///
    /// * `BitcreditBill` - The first version of the bill
    ///
    pub fn get_first_version_bill(&self, bill_keys: &BillKeys) -> Result<BitcreditBill> {
        let first_block_data = &self.get_first_block();
        let decrypted_bytes = first_block_data.get_decrypted_block_bytes(bill_keys)?;
        let bill_first_version: BitcreditBill = from_slice(&decrypted_bytes)?;
        Ok(bill_first_version)
    }

    /// This function iterates over all the blocks in the blockchain, extracts the nodes
    /// from each block, and compiles a unique list of non-empty nodes. Duplicate nodes
    /// are ignored.
    ///
    /// # Returns
    /// `Vec<String>`:  
    /// - A vector containing the unique identifiers of nodes associated with the bill.
    ///
    pub fn get_all_nodes_from_bill(&self, bill_keys: &BillKeys) -> Result<Vec<String>> {
        let mut nodes: Vec<String> = Vec::new();

        for block in &self.blocks {
            let nodes_in_block = block.get_nodes_from_block(bill_keys)?;
            for node in nodes_in_block {
                if !node.is_empty() && !nodes.contains(&node) {
                    nodes.push(node);
                }
            }
        }
        Ok(nodes)
    }

    /// This function determines the drawer of the bill by evaluating the following conditions:
    /// 1. If the drawer's name is not empty, it directly returns the drawer.
    /// 2. If the bill is directed to the payee (`to_payee` is `true`), it assigns the payee as the drawer.
    /// 3. Otherwise, the drawee is assigned as the drawer.
    ///
    /// # Returns
    /// `IdentityPublicData`:  
    /// - The identity data of the drawer, payee, or drawee depending on the evaluated conditions.
    ///
    pub fn get_drawer(&self, bill_keys: &BillKeys) -> Result<IdentityPublicData> {
        let drawer: IdentityPublicData;
        let bill = self.get_first_version_bill(bill_keys)?;
        if !bill.drawer.name.is_empty() {
            drawer = bill.drawer.clone();
        } else if bill.to_payee {
            drawer = bill.payee.clone();
        } else {
            drawer = bill.drawee.clone();
        }
        Ok(drawer)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        blockchain::bill::test::get_baseline_identity,
        tests::test::{get_bill_keys, TEST_PUB_KEY},
        util::rsa,
    };
    use libp2p::PeerId;

    fn get_sell_block(peer_id: String, prevhash: String) -> BillBlock {
        let mut buyer = IdentityPublicData::new_empty();
        buyer.peer_id = peer_id.clone();
        let mut seller = IdentityPublicData::new_empty();
        let endorser_peer_id = PeerId::random().to_string();
        seller.peer_id = endorser_peer_id.clone();
        let hashed_buyer = hex::encode(serde_json::to_vec(&buyer).unwrap());
        let hashed_seller = hex::encode(serde_json::to_vec(&seller).unwrap());

        let data = format!(
            "{}{}{}{}{}{}",
            SOLD_TO, &hashed_buyer, SOLD_BY, &hashed_seller, AMOUNT, "5000"
        );

        BillBlock::new(
            2,
            prevhash,
            hex::encode(rsa::encrypt_bytes_with_public_key(data.as_bytes(), TEST_PUB_KEY).unwrap()),
            BillOpCode::Sell,
            BcrKeys::new(),
            1731593928,
        )
        .unwrap()
    }

    #[test]
    fn validity_check_1_block_always_valid() {
        let bill = BitcreditBill::new_empty();
        let identity = get_baseline_identity();

        let chain = BillBlockchain::new(
            &bill,
            IdentityPublicData::new(identity.identity.clone(), identity.peer_id.to_string()),
            identity.key_pair,
            TEST_PUB_KEY.to_owned(),
            1731593928,
        )
        .unwrap();

        assert!(chain.is_chain_valid());
    }

    #[test]
    fn validity_check_2_blocks() {
        let bill = BitcreditBill::new_empty();
        let identity = get_baseline_identity();

        let mut chain = BillBlockchain::new(
            &bill,
            IdentityPublicData::new(identity.identity.clone(), identity.peer_id.to_string()),
            identity.key_pair,
            TEST_PUB_KEY.to_owned(),
            1731593928,
        )
        .unwrap();
        assert!(chain.try_add_block(get_sell_block(
            PeerId::random().to_string(),
            chain.get_first_block().hash.clone()
        ),));
        assert!(chain.is_chain_valid());
    }

    #[test]
    fn get_last_version_bill_last_endorsee_buyer() {
        let bill = BitcreditBill::new_empty();
        let identity = get_baseline_identity();

        let mut chain = BillBlockchain::new(
            &bill,
            IdentityPublicData::new(identity.identity.clone(), identity.peer_id.to_string()),
            identity.key_pair,
            TEST_PUB_KEY.to_owned(),
            1731593928,
        )
        .unwrap();
        let peer_id_last_endorsee = PeerId::random().to_string();
        assert!(chain.try_add_block(get_sell_block(
            peer_id_last_endorsee.clone(),
            chain.get_first_block().hash.clone()
        ),));

        let keys = get_bill_keys();
        let result = chain.get_last_version_bill(&keys);
        assert!(result.is_ok());
        assert_eq!(
            result.as_ref().unwrap().endorsee.peer_id,
            peer_id_last_endorsee
        );
    }

    #[test]
    fn is_last_sell_block_waiting_for_payment_deadline_passed() {
        let bill = BitcreditBill::new_empty();
        let identity = get_baseline_identity();

        let mut chain = BillBlockchain::new(
            &bill,
            IdentityPublicData::new(identity.identity.clone(), identity.peer_id.to_string()),
            identity.key_pair,
            TEST_PUB_KEY.to_owned(),
            1731593928,
        )
        .unwrap();
        let peer_id_last_endorsee = PeerId::random().to_string();
        assert!(chain.try_add_block(get_sell_block(
            peer_id_last_endorsee.clone(),
            chain.get_first_block().hash.clone()
        ),));

        let keys = get_bill_keys();
        let result = chain.is_last_sell_block_waiting_for_payment(&keys, 1751293728); // deadline
                                                                                      // passed
        assert!(result.is_ok());
        assert_eq!(result.as_ref().unwrap(), &WaitingForPayment::No);
    }

    #[test]
    fn is_last_sell_block_waiting_for_payment_baseline() {
        let bill = BitcreditBill::new_empty();
        let identity = get_baseline_identity();

        let mut chain = BillBlockchain::new(
            &bill,
            IdentityPublicData::new(identity.identity.clone(), identity.peer_id.to_string()),
            identity.key_pair,
            TEST_PUB_KEY.to_owned(),
            1731593928,
        )
        .unwrap();
        let peer_id_last_endorsee = PeerId::random().to_string();
        assert!(chain.try_add_block(get_sell_block(
            peer_id_last_endorsee.clone(),
            chain.get_first_block().hash.clone()
        ),));

        let keys = get_bill_keys();
        let result = chain.is_last_sell_block_waiting_for_payment(&keys, 1731593928);

        assert!(result.is_ok());
        if let WaitingForPayment::Yes(info) = result.unwrap() {
            assert_eq!(info.amount, 5000);
            assert_eq!(info.buyer.peer_id, peer_id_last_endorsee);
        } else {
            panic!("wrong result");
        }
    }

    #[test]
    fn get_all_nodes_from_bill_baseline() {
        let mut bill = BitcreditBill::new_empty();
        let identity = get_baseline_identity();
        bill.drawer =
            IdentityPublicData::new(identity.identity.clone(), identity.peer_id.to_string());

        let mut chain = BillBlockchain::new(
            &bill,
            IdentityPublicData::new(identity.identity.clone(), identity.peer_id.to_string()),
            identity.key_pair,
            TEST_PUB_KEY.to_owned(),
            1731593928,
        )
        .unwrap();
        let peer_id_last_endorsee = PeerId::random().to_string();
        assert!(chain.try_add_block(get_sell_block(
            peer_id_last_endorsee.clone(),
            chain.get_first_block().hash.clone()
        ),));

        let keys = get_bill_keys();
        let result = chain.get_all_nodes_from_bill(&keys);

        assert!(result.is_ok());
        assert_eq!(result.as_ref().unwrap().len(), 3); // drawer, buyer, seller
    }

    #[test]
    fn compare_chain_no_changes() {
        let bill = BitcreditBill::new_empty();
        let identity = get_baseline_identity();

        let mut chain = BillBlockchain::new(
            &bill,
            IdentityPublicData::new(identity.identity.clone(), identity.peer_id.to_string()),
            identity.key_pair,
            TEST_PUB_KEY.to_owned(),
            1731593928,
        )
        .unwrap();
        let chain2 = chain.clone();
        let peer_id_last_endorsee = PeerId::random().to_string();
        assert!(chain.try_add_block(get_sell_block(
            peer_id_last_endorsee.clone(),
            chain.get_first_block().hash.clone()
        ),));

        let result = chain.compare_chain(&chain2);

        assert!(!result);
    }

    #[test]
    fn compare_chain_changes() {
        let bill = BitcreditBill::new_empty();
        let identity = get_baseline_identity();

        let mut chain = BillBlockchain::new(
            &bill,
            IdentityPublicData::new(identity.identity.clone(), identity.peer_id.to_string()),
            identity.key_pair,
            TEST_PUB_KEY.to_owned(),
            1731593928,
        )
        .unwrap();
        let mut chain2 = chain.clone();
        let peer_id_last_endorsee = PeerId::random().to_string();
        assert!(chain.try_add_block(get_sell_block(
            peer_id_last_endorsee.clone(),
            chain.get_first_block().hash.clone()
        ),));

        let result = chain2.compare_chain(&chain);

        assert!(result);
        assert_eq!(chain.blocks.len(), chain2.blocks.len());
    }
}
