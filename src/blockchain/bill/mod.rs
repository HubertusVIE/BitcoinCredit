use borsh_derive::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{Blockchain, Result};
use crate::service::bill_service::BillKeys;
use crate::service::contact_service::IdentityPublicData;

mod block;
mod chain;

pub use block::BillBlock;
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
    Sell,
    Mint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WaitingForPayment {
    Yes(Box<PaymentInfo>),
    No,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaymentInfo {
    pub buyer: IdentityPublicData,
    pub seller: IdentityPublicData,
    pub amount: u64,
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
    pub timestamp: i64,
    pub data: String,
    pub previous_hash: String,
    pub signature: String,
    pub operation_code: BillOpCode,
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
            operation_code: block.operation_code,
            label,
        })
    }
}

fn extract_after_phrase(input: &str, phrase: &str) -> Option<String> {
    if let Some(start) = input.find(phrase) {
        let start_idx = start + phrase.len();
        if let Some(remaining) = input.get(start_idx..) {
            if let Some(end_idx) = remaining.find(' ') {
                return Some(remaining[..end_idx].to_string());
            } else {
                return Some(remaining.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        service::{
            bill_service::BitcreditBill,
            identity_service::{Identity, IdentityWithAll},
        },
        tests::test::{TEST_PRIVATE_KEY, TEST_PUB_KEY},
        util::BcrKeys,
    };
    use libp2p::PeerId;

    pub fn get_baseline_identity() -> IdentityWithAll {
        let mut identity = Identity::new_empty();
        identity.name = "drawer".to_owned();
        identity.public_key_pem = TEST_PUB_KEY.to_owned();
        identity.private_key_pem = TEST_PRIVATE_KEY.to_owned();
        IdentityWithAll {
            identity,
            node_id: PeerId::random(),
            key_pair: BcrKeys::new(),
        }
    }

    #[test]
    fn start_blockchain_for_new_bill_baseline() {
        let bill = BitcreditBill::new_empty();
        let identity = get_baseline_identity();

        let result = BillBlockchain::new(
            &bill,
            IdentityPublicData::new(identity.identity.clone(), identity.node_id.to_string()),
            identity.key_pair,
            TEST_PUB_KEY.to_owned(),
            1731593928,
        );

        assert!(result.is_ok());
        assert_eq!(result.as_ref().unwrap().blocks().len(), 1);
    }

    #[test]
    fn extract_after_phrase_basic() {
        assert_eq!(
            extract_after_phrase(
                "Endorsed by 123 endorsed to 456 amount: 5000",
                "Endorsed by "
            ),
            Some(String::from("123"))
        );
        assert_eq!(
            extract_after_phrase(
                "Endorsed by 123 endorsed to 456 amount: 5000",
                " endorsed to "
            ),
            Some(String::from("456"))
        );
        assert_eq!(
            extract_after_phrase("Endorsed by 123 endorsed to 456 amount: 5000", " amount: "),
            Some(String::from("5000"))
        );
        assert_eq!(
            extract_after_phrase(
                "Endorsed by 123 endorsed to 456 amount: 5000",
                " weird stuff "
            ),
            None
        );
    }
}
