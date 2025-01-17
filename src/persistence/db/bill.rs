use super::Result;
use crate::{
    blockchain::bill::BillOpCode,
    constants::{DB_BILL_ID, DB_OP_CODE, DB_TABLE, DB_TIMESTAMP, PAYMENT_DEADLINE_SECONDS},
    persistence::{bill::BillStoreApi, Error},
    service::bill_service::BillKeys,
    util,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use surrealdb::{engine::any::Any, sql::Thing, Surreal};

#[derive(Clone)]
pub struct SurrealBillStore {
    db: Surreal<Any>,
}

impl SurrealBillStore {
    const CHAIN_TABLE: &'static str = "bill_chain";
    const KEYS_TABLE: &'static str = "bill_keys";
    const PAID_TABLE: &'static str = "bill_paid";

    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl BillStoreApi for SurrealBillStore {
    async fn exists(&self, id: &str) -> bool {
        match self
            .db
            .query(
                "SELECT bill_id FROM type::table($table) WHERE bill_id = $bill_id GROUP BY bill_id",
            )
            .bind((DB_TABLE, Self::CHAIN_TABLE))
            .bind((DB_BILL_ID, id.to_owned()))
            .await
        {
            Ok(mut res) => {
                res.take::<Option<BillIdDb>>(0)
                    .map(|_| true)
                    .unwrap_or(false)
                    && self.get_keys(id).await.map(|_| true).unwrap_or(false)
            }
            Err(_) => false,
        }
    }

    async fn get_ids(&self) -> Result<Vec<String>> {
        let ids: Vec<BillIdDb> = self
            .db
            .query("SELECT bill_id FROM type::table($table) GROUP BY bill_id")
            .bind((DB_TABLE, Self::CHAIN_TABLE))
            .await?
            .take(0)?;
        Ok(ids.into_iter().map(|b| b.bill_id).collect())
    }

    async fn save_keys(&self, id: &str, key_pair: &BillKeys) -> Result<()> {
        let entity: BillKeysDb = key_pair.into();
        let _: Option<BillKeysDb> = self
            .db
            .create((Self::KEYS_TABLE, id))
            .content(entity)
            .await?;
        Ok(())
    }

    async fn get_keys(&self, id: &str) -> Result<BillKeys> {
        let result: Option<BillKeysDb> = self.db.select((Self::KEYS_TABLE, id)).await?;
        match result {
            None => Err(Error::NoSuchEntity("bill".to_string(), id.to_owned())),
            Some(c) => Ok(c.into()),
        }
    }

    async fn is_paid(&self, id: &str) -> Result<bool> {
        let result: Option<BillPaidDb> = self.db.select((Self::PAID_TABLE, id)).await?;
        Ok(result.is_some())
    }

    async fn set_to_paid(&self, id: &str, payment_address: &str) -> Result<()> {
        let entity = BillPaidDb {
            id: (Self::PAID_TABLE, id).into(),
            payment_address: payment_address.to_string(),
        };
        let _: Option<BillPaidDb> = self
            .db
            .upsert((Self::PAID_TABLE, id))
            .content(entity)
            .await?;
        Ok(())
    }

    async fn get_bill_ids_waiting_for_payment(&self) -> Result<Vec<String>> {
        let bill_ids_paid: Vec<BillPaidDb> = self.db.select(Self::PAID_TABLE).await?;
        let with_req_to_pay_bill_ids: Vec<BillIdDb> = self
            .db
            .query(
                "SELECT bill_id FROM type::table($table) WHERE op_code = $op_code GROUP BY bill_id",
            )
            .bind((DB_TABLE, Self::CHAIN_TABLE))
            .bind((DB_OP_CODE, BillOpCode::RequestToPay))
            .await?
            .take(0)?;
        let result: Vec<String> = with_req_to_pay_bill_ids
            .into_iter()
            .filter_map(|bid| {
                if !bill_ids_paid
                    .iter()
                    .any(|idp| idp.id.id.to_raw() == bid.bill_id)
                {
                    Some(bid.bill_id)
                } else {
                    None
                }
            })
            .collect();
        Ok(result)
    }

    async fn get_bill_ids_waiting_for_sell_payment(&self) -> Result<Vec<String>> {
        let timestamp_now_minus_payment_deadline =
            util::date::now().timestamp() - PAYMENT_DEADLINE_SECONDS as i64;
        let query = r#"SELECT bill_id FROM 
            (SELECT bill_id, math::max(block_id) as block_id, op_code, timestamp FROM type::table($table) GROUP BY bill_id)
            .map(|$v| {
                (SELECT bill_id, block_id, op_code, timestamp FROM bill_chain WHERE bill_id = $v.bill_id AND block_id = $v.block_id)[0]
            })
            .flatten() WHERE timestamp > $timestamp AND op_code = $op_code"#;
        let result: Vec<BillIdDb> = self
            .db
            .query(query)
            .bind((DB_TABLE, Self::CHAIN_TABLE))
            .bind((DB_TIMESTAMP, timestamp_now_minus_payment_deadline))
            .bind((DB_OP_CODE, BillOpCode::OfferToSell))
            .await?
            .take(0)?;
        Ok(result.into_iter().map(|bid| bid.bill_id).collect())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillPaidDb {
    pub id: Thing,
    pub payment_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillIdDb {
    pub bill_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillKeysDb {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub public_key: String,
    pub private_key: String,
}

impl From<BillKeysDb> for BillKeys {
    fn from(value: BillKeysDb) -> Self {
        Self {
            public_key: value.public_key,
            private_key: value.private_key,
        }
    }
}

impl From<&BillKeys> for BillKeysDb {
    fn from(value: &BillKeys) -> Self {
        Self {
            id: None,
            public_key: value.public_key.clone(),
            private_key: value.private_key.clone(),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::SurrealBillStore;
    use crate::{
        blockchain::bill::{
            block::{
                BillIssueBlockData, BillOfferToSellBlockData, BillRequestToPayBlockData,
                BillSoldBlockData,
            },
            tests::get_baseline_identity,
            BillBlock,
        },
        persistence::{
            bill::{BillChainStoreApi, BillStoreApi},
            db::{bill_chain::SurrealBillChainStore, get_memory_db},
        },
        service::{
            bill_service::{BillKeys, BitcreditBill},
            contact_service::IdentityPublicData,
        },
        tests::tests::{get_bill_keys, TEST_PRIVATE_KEY_SECP, TEST_PUB_KEY_SECP},
        util::{self, BcrKeys},
    };
    use chrono::Months;
    use surrealdb::{engine::any::Any, Surreal};

    async fn get_db() -> Surreal<Any> {
        get_memory_db("test", "bill")
            .await
            .expect("could not create memory db")
    }
    async fn get_store(mem_db: Surreal<Any>) -> SurrealBillStore {
        SurrealBillStore::new(mem_db)
    }

    async fn get_chain_store(mem_db: Surreal<Any>) -> SurrealBillChainStore {
        SurrealBillChainStore::new(mem_db)
    }

    pub fn get_first_block(id: &str) -> BillBlock {
        let mut bill = BitcreditBill::new_empty();
        bill.maturity_date = "2099-05-05".to_string();
        bill.id = id.to_owned();
        bill.drawer = IdentityPublicData::new_only_node_id(BcrKeys::new().get_public_key());
        bill.payee = bill.drawer.clone();
        bill.drawee = IdentityPublicData::new_only_node_id(BcrKeys::new().get_public_key());

        BillBlock::create_block_for_issue(
            id.to_owned(),
            String::from("prevhash"),
            &BillIssueBlockData::from(bill, None, 1731593928),
            &get_baseline_identity().key_pair,
            None,
            &BcrKeys::from_private_key(&get_bill_keys().private_key).unwrap(),
            1731593928,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_exists() {
        let db = get_db().await;
        let chain_store = get_chain_store(db.clone()).await;
        let store = get_store(db.clone()).await;
        assert!(!store.exists("1234").await);
        chain_store
            .add_block("1234", &get_first_block("1234"))
            .await
            .unwrap();
        assert!(!store.exists("1234").await);
        store
            .save_keys(
                "1234",
                &BillKeys {
                    private_key: TEST_PRIVATE_KEY_SECP.to_string(),
                    public_key: TEST_PUB_KEY_SECP.to_string(),
                },
            )
            .await
            .unwrap();
        assert!(store.exists("1234").await)
    }

    #[tokio::test]
    async fn test_get_ids() {
        let db = get_db().await;
        let chain_store = get_chain_store(db.clone()).await;
        let store = get_store(db.clone()).await;
        chain_store
            .add_block("1234", &get_first_block("1234"))
            .await
            .unwrap();
        chain_store
            .add_block("4321", &get_first_block("4321"))
            .await
            .unwrap();
        let res = store.get_ids().await;
        assert!(res.is_ok());
        assert!(res.as_ref().unwrap().contains(&"1234".to_string()));
        assert!(res.as_ref().unwrap().contains(&"4321".to_string()));
    }

    #[tokio::test]
    async fn test_save_get_keys() {
        let store = get_store(get_db().await).await;
        let res = store
            .save_keys(
                "1234",
                &BillKeys {
                    private_key: TEST_PRIVATE_KEY_SECP.to_owned(),
                    public_key: TEST_PUB_KEY_SECP.to_owned(),
                },
            )
            .await;
        assert!(res.is_ok());
        let get_res = store.get_keys("1234").await;
        assert!(get_res.is_ok());
        assert_eq!(get_res.as_ref().unwrap().private_key, TEST_PRIVATE_KEY_SECP);
    }

    #[tokio::test]
    async fn test_paid() {
        let store = get_store(get_db().await).await;
        let res = store.set_to_paid("1234", "1234paymentaddress").await;
        assert!(res.is_ok());
        let get_res = store.is_paid("1234").await;
        assert!(get_res.is_ok());
        assert!(get_res.as_ref().unwrap());

        // save again
        let res_again = store.set_to_paid("1234", "1234paymentaddress").await;
        assert!(res_again.is_ok());
        let get_res_again = store.is_paid("1234").await;
        assert!(get_res_again.is_ok());
        assert!(get_res_again.as_ref().unwrap());

        // different bill without paid state
        let get_res_not_paid = store.is_paid("4321").await;
        assert!(get_res_not_paid.is_ok());
        assert!(!get_res_not_paid.as_ref().unwrap());
    }

    #[tokio::test]
    async fn test_bills_waiting_for_payment() {
        let db = get_db().await;
        let chain_store = get_chain_store(db.clone()).await;
        let store = get_store(db.clone()).await;

        let first_block = get_first_block("1234");
        chain_store
            .add_block("4321", &get_first_block("4321"))
            .await
            .unwrap(); // not returned, no req to pay block
        chain_store.add_block("1234", &first_block).await.unwrap();
        chain_store
            .add_block(
                "1234",
                &BillBlock::create_block_for_request_to_pay(
                    "1234".to_string(),
                    &first_block,
                    &BillRequestToPayBlockData {
                        requester: IdentityPublicData::new_only_node_id(
                            BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP)
                                .unwrap()
                                .get_public_key(),
                        )
                        .into(),
                        currency_code: "sat".to_string(),
                        signatory: None,
                        signing_timestamp: 1731593928,
                        signing_address: "some_address".to_string(),
                    },
                    &BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP).unwrap(),
                    None,
                    &BcrKeys::from_private_key(&get_bill_keys().private_key).unwrap(),
                    1731593928,
                )
                .unwrap(),
            )
            .await
            .unwrap();

        let res = store.get_bill_ids_waiting_for_payment().await;
        assert!(res.is_ok());
        assert_eq!(res.as_ref().unwrap().len(), 1);

        // add the bill to paid, expect it not to be returned afterwards
        store
            .set_to_paid("1234", "1234paymentaddress")
            .await
            .unwrap();

        let res_after_paid = store.get_bill_ids_waiting_for_payment().await;
        assert!(res_after_paid.is_ok());
        assert_eq!(res_after_paid.as_ref().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_bills_waiting_for_payment_offer_to_sell() {
        let db = get_db().await;
        let chain_store = get_chain_store(db.clone()).await;
        let store = get_store(db.clone()).await;
        let now = util::date::now().timestamp() as u64;

        let first_block = get_first_block("1234");
        chain_store
            .add_block("4321", &get_first_block("4321"))
            .await
            .unwrap(); // not returned, no offer to sell block
        chain_store.add_block("1234", &first_block).await.unwrap();
        let second_block = BillBlock::create_block_for_offer_to_sell(
            "1234".to_string(),
            &first_block,
            &BillOfferToSellBlockData {
                seller: IdentityPublicData::new_only_node_id(
                    BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP)
                        .unwrap()
                        .get_public_key(),
                )
                .into(),
                buyer: IdentityPublicData::new_only_node_id(BcrKeys::new().get_public_key()).into(),
                currency_code: "sat".to_string(),
                amount: 15000,
                payment_address: "1234paymentaddress".to_string(),
                signatory: None,
                signing_timestamp: now,
                signing_address: "some_address".to_string(),
            },
            &BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP).unwrap(),
            None,
            &BcrKeys::from_private_key(&get_bill_keys().private_key).unwrap(),
            now,
        )
        .unwrap();
        chain_store.add_block("1234", &second_block).await.unwrap();

        let res = store.get_bill_ids_waiting_for_sell_payment().await;
        assert!(res.is_ok());
        assert_eq!(res.as_ref().unwrap().len(), 1);

        chain_store
            .add_block(
                "1234",
                &BillBlock::create_block_for_sold(
                    "1234".to_string(),
                    &second_block,
                    &BillSoldBlockData {
                        seller: IdentityPublicData::new_only_node_id(
                            BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP)
                                .unwrap()
                                .get_public_key(),
                        )
                        .into(),
                        buyer: IdentityPublicData::new_only_node_id(
                            BcrKeys::new().get_public_key(),
                        )
                        .into(),
                        currency_code: "sat".to_string(),
                        amount: 15000,
                        payment_address: "1234paymentaddress".to_string(),
                        signatory: None,
                        signing_timestamp: now,
                        signing_address: "some_address".to_string(),
                    },
                    &BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP).unwrap(),
                    None,
                    &BcrKeys::from_private_key(&get_bill_keys().private_key).unwrap(),
                    now,
                )
                .unwrap(),
            )
            .await
            .unwrap();

        // add sold block, shouldn't return anymore
        let res_after_sold = store.get_bill_ids_waiting_for_sell_payment().await;
        assert!(res_after_sold.is_ok());
        assert_eq!(res_after_sold.as_ref().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_bills_waiting_for_payment_offer_to_sell_expired() {
        let db = get_db().await;
        let chain_store = get_chain_store(db.clone()).await;
        let store = get_store(db.clone()).await;
        let now_minus_one_month = util::date::now()
            .checked_sub_months(Months::new(1))
            .unwrap()
            .timestamp() as u64;

        let first_block = get_first_block("1234");
        chain_store
            .add_block("4321", &get_first_block("4321"))
            .await
            .unwrap(); // not returned, no offer to sell block
        chain_store.add_block("1234", &first_block).await.unwrap();
        let second_block = BillBlock::create_block_for_offer_to_sell(
            "1234".to_string(),
            &first_block,
            &BillOfferToSellBlockData {
                seller: IdentityPublicData::new_only_node_id(
                    BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP)
                        .unwrap()
                        .get_public_key(),
                )
                .into(),
                buyer: IdentityPublicData::new_only_node_id(BcrKeys::new().get_public_key()).into(),
                currency_code: "sat".to_string(),
                amount: 15000,
                payment_address: "1234paymentaddress".to_string(),
                signatory: None,
                signing_timestamp: now_minus_one_month,
                signing_address: "some_address".to_string(),
            },
            &BcrKeys::from_private_key(TEST_PRIVATE_KEY_SECP).unwrap(),
            None,
            &BcrKeys::from_private_key(&get_bill_keys().private_key).unwrap(),
            now_minus_one_month,
        )
        .unwrap();
        chain_store.add_block("1234", &second_block).await.unwrap();

        // nothing gets returned, because the offer to sell is expired
        let res = store.get_bill_ids_waiting_for_sell_payment().await;
        assert!(res.is_ok());
        assert_eq!(res.as_ref().unwrap().len(), 0);
    }
}
