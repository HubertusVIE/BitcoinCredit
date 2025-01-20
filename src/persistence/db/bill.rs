use super::Result;
use crate::{
    constants::{DB_BILL_ID, DB_TABLE},
    persistence::{bill::BillStoreApi, Error},
    service::bill_service::BillKeys,
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
                "SELECT bill_id from type::table($table) WHERE bill_id = $bill_id GROUP BY bill_id",
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
            .query("SELECT bill_id from type::table($table) GROUP BY bill_id")
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
        blockchain::bill::{block::BillIssueBlockData, tests::get_baseline_identity, BillBlock},
        persistence::{
            bill::{BillChainStoreApi, BillStoreApi},
            db::{bill_chain::SurrealBillChainStore, get_memory_db},
        },
        service::{
            bill_service::{BillKeys, BitcreditBill},
            contact_service::IdentityPublicData,
        },
        tests::tests::{get_bill_keys, TEST_PRIVATE_KEY_SECP, TEST_PUB_KEY_SECP},
        util::BcrKeys,
    };
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
}
