use borsh_derive::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{Blockchain, Result};
use crate::service::bill_service::BillKeys;

pub mod block;
pub mod chain;

pub use block::BillBlock;
use block::BillIdentityBlockData;
pub use chain::BillBlockchain;

#[derive(
    BorshSerialize, BorshDeserialize, Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema,
)]
pub enum BillOpCode {
    Issue,
    Accept,
    Endorse,
    RequestToAccept,
    RequestToPay,
    OfferToSell,
    Sold,
    Mint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WaitingForPayment {
    Yes(Box<PaymentInfo>),
    No,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaymentInfo {
    pub buyer: BillIdentityBlockData,
    pub seller: BillIdentityBlockData,
    pub amount: u64,
    pub currency_code: String,
    pub payment_address: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct BillBlockchainToReturn {
    pub blocks: Vec<BillBlockToReturn>,
}

impl BillBlockchainToReturn {
    /// Creates a new blockchain to return by transforming a given blockchain into its corresponding representation.
    ///
    /// # Parameters
    /// * `chain` - The blockchain to be transformed. It contains the list of blocks and the initial bill version
    ///   necessary for processing.
    /// * `bill_keys` - The keys for the bill
    ///
    /// # Returns
    /// A new instance containing the transformed `BillBlockToReturn` objects.
    ///
    pub fn new(chain: BillBlockchain, bill_keys: &BillKeys) -> Result<Self> {
        let mut blocks: Vec<BillBlockToReturn> = Vec::new();
        for block in chain.blocks() {
            blocks.push(BillBlockToReturn::new(block.clone(), bill_keys)?);
        }
        Ok(Self { blocks })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct BillBlockToReturn {
    pub id: u64,
    pub hash: String,
    pub timestamp: u64,
    pub data: String,
    pub previous_hash: String,
    pub signature: String,
    pub op_code: BillOpCode,
    pub label: String,
}

impl BillBlockToReturn {
    /// Creates a new block to return for the given bill, with an attached history label,
    /// describing what happened in this block
    pub fn new(block: BillBlock, bill_keys: &BillKeys) -> Result<Self> {
        let label = block.get_history_label(bill_keys)?;

        Ok(Self {
            id: block.id,
            hash: block.hash,
            timestamp: block.timestamp,
            data: block.data,
            previous_hash: block.previous_hash,
            signature: block.signature,
            op_code: block.op_code,
            label,
        })
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        service::{
            bill_service::BitcreditBill,
            identity_service::{Identity, IdentityWithAll},
        },
        tests::tests::TEST_PRIVATE_KEY_SECP,
        util::BcrKeys,
    };
    use block::BillIssueBlockData;

    pub fn get_baseline_identity() -> IdentityWithAll {
        let keys = BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP).unwrap();
        let mut identity = Identity::new_empty();
        identity.node_id = keys.get_public_key();
        identity.name = "drawer".to_owned();
        IdentityWithAll {
            identity,
            key_pair: keys,
        }
    }

    #[test]
    fn start_blockchain_for_new_bill_baseline() {
        let bill = BitcreditBill::new_empty();
        let identity = get_baseline_identity();

        let result = BillBlockchain::new(
            &BillIssueBlockData::from(bill, None, 1731593928),
            identity.key_pair,
            None,
            BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP).unwrap(),
            1731593928,
        );

        assert!(result.is_ok());
        assert_eq!(result.as_ref().unwrap().blocks().len(), 1);
    }
}
