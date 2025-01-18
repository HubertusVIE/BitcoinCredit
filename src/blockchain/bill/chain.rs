use super::super::Result;
use super::block::{
    BillBlock, BillEndorseBlockData, BillIdentityBlockData, BillIssueBlockData, BillMintBlockData,
    BillSellBlockData,
};
use super::BillOpCode;
use super::BillOpCode::{Endorse, Mint, Sell};
use super::PaymentInfo;
use super::WaitingForPayment;
use crate::blockchain::Blockchain;
use crate::service::bill_service::{BillKeys, BitcreditBill};
use crate::service::contact_service::IdentityPublicData;
use crate::util::{self, BcrKeys};
use crate::web::data::File;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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

#[derive(Debug, Clone)]
pub struct LastVersionBill {
    pub id: String,
    pub bill_jurisdiction: String,
    pub timestamp_at_drawing: u64,
    pub drawee: BillIdentityBlockData,
    pub drawer: BillIdentityBlockData,
    pub payee: BillIdentityBlockData,
    pub endorsee: Option<BillIdentityBlockData>,
    pub place_of_drawing: String,
    pub currency_code: String,
    pub amount_numbers: u64,
    pub amounts_letters: String,
    pub maturity_date: String,
    pub date_of_issue: String,
    pub place_of_payment: String,
    pub language: String,
    pub files: Vec<File>,
}

impl BillBlockchain {
    /// Creates a new blockchain for the given bill, encrypting the metadata using the bill's public
    /// key
    pub fn new(
        bill: &BillIssueBlockData,
        drawer_key_pair: BcrKeys,
        drawer_company_key_pair: Option<BcrKeys>,
        bill_keys: BcrKeys,
        timestamp: u64,
    ) -> Result<Self> {
        let genesis_hash = util::base58_encode(bill.id.as_bytes());

        let first_block = BillBlock::create_block_for_issue(
            bill.id.clone(),
            genesis_hash,
            bill,
            &drawer_key_pair,
            drawer_company_key_pair.as_ref(),
            &bill_keys,
            timestamp,
        )?;

        Ok(Self {
            blocks: vec![first_block],
        })
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
    /// A `LastVersionBill` object containing the most recent version of the bill, including the payee, endorsee,
    /// and other associated information.
    ///
    pub fn get_last_version_bill(&self, bill_keys: &BillKeys) -> Result<LastVersionBill> {
        let first_block = self.get_first_block();
        let bill_first_version: BillIssueBlockData =
            first_block.get_decrypted_block_bytes(bill_keys)?;

        let mut last_endorsee = None;

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
                let block_data_decrypted: BillSellBlockData =
                    last_version_block_sell.get_decrypted_block_bytes(bill_keys)?;
                let buyer = block_data_decrypted.buyer;

                last_endorsee = Some(buyer);
            } else if self.block_with_operation_code_exists(Endorse.clone())
                && (last_version_block_endorse.id > last_version_block_mint.id)
            {
                let block_data_decrypted: BillEndorseBlockData =
                    last_version_block_endorse.get_decrypted_block_bytes(bill_keys)?;
                let endorsee = block_data_decrypted.endorsee;

                last_endorsee = Some(endorsee);
            } else if self.block_with_operation_code_exists(Mint.clone())
                && (last_version_block_mint.id > last_version_block_endorse.id)
            {
                let block_data_decrypted: BillMintBlockData =
                    last_version_block_mint.get_decrypted_block_bytes(bill_keys)?;
                let mint = block_data_decrypted.endorsee;

                last_endorsee = Some(mint);
            }
        }

        let mut payee = bill_first_version.payee.into();

        if let Some(ref endorsee) = last_endorsee {
            payee = endorsee.clone();
        }

        Ok(LastVersionBill {
            id: bill_first_version.id,
            bill_jurisdiction: bill_first_version.bill_jurisdiction,
            timestamp_at_drawing: bill_first_version.timestamp_at_drawing,
            drawee: bill_first_version.drawee.into(),
            drawer: bill_first_version.drawer.into(),
            payee: payee.clone(),
            endorsee: last_endorsee.clone(),
            place_of_drawing: bill_first_version.place_of_drawing,
            currency_code: bill_first_version.currency_code,
            amount_numbers: bill_first_version.amount_numbers,
            amounts_letters: bill_first_version.amounts_letters,
            maturity_date: bill_first_version.maturity_date,
            date_of_issue: bill_first_version.date_of_issue,
            place_of_payment: bill_first_version.place_of_payment,
            language: bill_first_version.language,
            files: bill_first_version.files,
        })
    }

    /// Checks if the last block is a sell block, if it's deadline is still active and if so,
    /// returns the buyer, seller and amount
    pub fn is_last_sell_block_waiting_for_payment(
        &self,
        bill_keys: &BillKeys,
        current_timestamp: u64,
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

            let block_data_decrypted: BillSellBlockData =
                last_version_block_sell.get_decrypted_block_bytes(bill_keys)?;
            Ok(WaitingForPayment::Yes(Box::new(PaymentInfo {
                buyer: block_data_decrypted.buyer,
                seller: block_data_decrypted.seller,
                amount: block_data_decrypted.amount,
                currency_code: block_data_decrypted.currency_code,
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
    fn check_if_payment_deadline_has_passed(&self, current_timestamp: u64) -> bool {
        if self.block_with_operation_code_exists(Sell) {
            let last_version_block_sell = self.get_last_version_block_with_op_code(Sell);
            let timestamp = last_version_block_sell.timestamp;

            let period: u64 = (86400 * 2) as u64; // 2 days deadline
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
        let bill_first_version: BillIssueBlockData =
            first_block_data.get_decrypted_block_bytes(bill_keys)?;
        Ok(bill_first_version.into())
    }

    /// This function iterates over all the blocks in the blockchain, extracts the nodes
    /// from each block, and compiles a unique list of nodes.
    ///
    /// # Returns
    /// `Vec<String>`:
    /// - A vector containing the unique identifiers of nodes associated with the bill.
    ///
    pub fn get_all_nodes_from_bill(&self, bill_keys: &BillKeys) -> Result<Vec<String>> {
        let mut nodes: HashSet<String> = HashSet::new();

        for block in &self.blocks {
            let nodes_in_block = block.get_nodes_from_block(bill_keys)?;
            for node in nodes_in_block {
                nodes.insert(node);
            }
        }
        Ok(nodes.into_iter().collect())
    }

    /// This function determines the drawer of the bill
    ///
    /// # Returns
    /// `IdentityPublicData`:
    /// - The identity data of the drawer
    ///
    pub fn get_drawer(&self, bill_keys: &BillKeys) -> Result<IdentityPublicData> {
        let bill = self.get_first_version_bill(bill_keys)?;
        Ok(bill.drawer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        blockchain::bill::tests::get_baseline_identity,
        tests::tests::{get_bill_keys, TEST_PRIVATE_KEY_SECP},
    };

    fn get_sell_block(
        buyer_node_id: String,
        seller_node_id: String,
        previous_block: &BillBlock,
    ) -> BillBlock {
        let buyer = IdentityPublicData::new_only_node_id(buyer_node_id);
        let seller = IdentityPublicData::new_only_node_id(seller_node_id);

        BillBlock::create_block_for_sell(
            "some id".to_string(),
            previous_block,
            &BillSellBlockData {
                buyer: buyer.clone().into(),
                seller: seller.clone().into(),
                amount: 5000,
                currency_code: "sat".to_string(),
                signatory: None,
                signing_timestamp: 1731593928,
                signing_address: seller.postal_address,
            },
            &get_baseline_identity().key_pair,
            None,
            &BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP).unwrap(),
            1731593928,
        )
        .unwrap()
    }

    #[test]
    fn validity_check_1_block_always_valid() {
        let bill = BitcreditBill::new_empty();
        let identity = get_baseline_identity();

        let chain = BillBlockchain::new(
            &BillIssueBlockData::from(bill, None),
            identity.key_pair,
            None,
            BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP).unwrap(),
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
            &BillIssueBlockData::from(bill, None),
            identity.key_pair,
            None,
            BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP).unwrap(),
            1731593928,
        )
        .unwrap();
        assert!(chain.try_add_block(get_sell_block(
            BcrKeys::new().get_public_key(),
            identity.identity.node_id,
            chain.get_first_block()
        ),));
        assert!(chain.is_chain_valid());
    }

    #[test]
    fn get_last_version_bill_last_endorsee_buyer() {
        let bill = BitcreditBill::new_empty();
        let identity = get_baseline_identity();

        let mut chain = BillBlockchain::new(
            &BillIssueBlockData::from(bill, None),
            identity.key_pair,
            None,
            BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP).unwrap(),
            1731593928,
        )
        .unwrap();
        let node_id_last_endorsee = BcrKeys::new().get_public_key();
        assert!(chain.try_add_block(get_sell_block(
            node_id_last_endorsee.clone(),
            identity.identity.node_id,
            chain.get_first_block()
        ),));

        let keys = get_bill_keys();
        let result = chain.get_last_version_bill(&keys);
        assert!(result.is_ok());
        assert_eq!(
            result.as_ref().unwrap().endorsee.as_ref().unwrap().node_id,
            node_id_last_endorsee
        );
    }

    #[test]
    fn is_last_sell_block_waiting_for_payment_deadline_passed() {
        let bill = BitcreditBill::new_empty();
        let identity = get_baseline_identity();

        let mut chain = BillBlockchain::new(
            &BillIssueBlockData::from(bill, None),
            identity.key_pair,
            None,
            BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP).unwrap(),
            1731593928,
        )
        .unwrap();
        let node_id_last_endorsee = BcrKeys::new().get_public_key();
        assert!(chain.try_add_block(get_sell_block(
            node_id_last_endorsee.clone(),
            identity.identity.node_id,
            chain.get_first_block()
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
            &BillIssueBlockData::from(bill, None),
            identity.key_pair,
            None,
            BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP).unwrap(),
            1731593928,
        )
        .unwrap();
        let node_id_last_endorsee = BcrKeys::new().get_public_key();
        assert!(chain.try_add_block(get_sell_block(
            node_id_last_endorsee.clone(),
            identity.identity.node_id,
            chain.get_first_block()
        ),));

        let keys = get_bill_keys();
        let result = chain.is_last_sell_block_waiting_for_payment(&keys, 1731593928);

        assert!(result.is_ok());
        if let WaitingForPayment::Yes(info) = result.unwrap() {
            assert_eq!(info.amount, 5000);
            assert_eq!(info.buyer.node_id, node_id_last_endorsee);
        } else {
            panic!("wrong result");
        }
    }

    #[test]
    fn get_all_nodes_from_bill_baseline() {
        let mut bill = BitcreditBill::new_empty();
        let identity = get_baseline_identity();
        bill.drawer = IdentityPublicData::new(identity.identity.clone());
        bill.drawee = IdentityPublicData::new(identity.identity.clone());
        bill.payee = IdentityPublicData::new_only_node_id(BcrKeys::new().get_public_key());

        let mut chain = BillBlockchain::new(
            &BillIssueBlockData::from(bill, None),
            identity.key_pair,
            None,
            BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP).unwrap(),
            1731593928,
        )
        .unwrap();
        let node_id_last_endorsee = BcrKeys::new().get_public_key();
        assert!(chain.try_add_block(get_sell_block(
            node_id_last_endorsee.clone(),
            identity.identity.node_id,
            chain.get_first_block()
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
            &BillIssueBlockData::from(bill, None),
            identity.key_pair,
            None,
            BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP).unwrap(),
            1731593928,
        )
        .unwrap();
        let chain2 = chain.clone();
        let node_id_last_endorsee = BcrKeys::new().get_public_key();
        assert!(chain.try_add_block(get_sell_block(
            node_id_last_endorsee.clone(),
            identity.identity.node_id,
            chain.get_first_block()
        ),));

        let result = chain.compare_chain(&chain2);

        assert!(!result);
    }

    #[test]
    fn compare_chain_changes() {
        let bill = BitcreditBill::new_empty();
        let identity = get_baseline_identity();

        let mut chain = BillBlockchain::new(
            &BillIssueBlockData::from(bill, None),
            identity.key_pair,
            None,
            BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP).unwrap(),
            1731593928,
        )
        .unwrap();
        let mut chain2 = chain.clone();
        let node_id_last_endorsee = BcrKeys::new().get_public_key();
        assert!(chain.try_add_block(get_sell_block(
            node_id_last_endorsee.clone(),
            identity.identity.node_id,
            chain.get_first_block()
        ),));

        let result = chain2.compare_chain(&chain);

        assert!(result);
        assert_eq!(chain.blocks.len(), chain2.blocks.len());
    }
}
