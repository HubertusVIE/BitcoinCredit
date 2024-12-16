use super::super::{Error, Result};
use crate::{
    blockchain::{
        identity::{IdentityBlock, IdentityOpCode},
        Block,
    },
    persistence::identity_chain::IdentityChainStoreApi,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use surrealdb::{engine::any::Any, Surreal};

#[derive(Clone)]
pub struct SurrealIdentityChainStore {
    db: Surreal<Any>,
}

impl SurrealIdentityChainStore {
    const TABLE: &'static str = "identity_chain";

    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl IdentityChainStoreApi for SurrealIdentityChainStore {
    async fn get_latest_block(&self) -> Result<IdentityBlock> {
        let result: Vec<IdentityBlockDb> = self
            .db
            .query("SELECT * FROM type::table($table) ORDER BY block_id DESC LIMIT 1")
            .bind(("table", Self::TABLE))
            .await?
            .take(0)?;

        match result.first() {
            None => Err(Error::NoIdentityBlock),
            Some(block) => Ok(block.to_owned().into()),
        }
    }

    async fn add_block(&self, block: &IdentityBlock) -> Result<()> {
        let entity: IdentityBlockDb = block.into();
        match self.get_latest_block().await {
            Err(Error::NoIdentityBlock) => {
                // if there is no latest block, ensure it's a valid first block
                if block.id == 1 && block.verify() && block.validate_hash() {
                    let _: Option<IdentityBlockDb> =
                        self.db.create(Self::TABLE).content(entity).await?;
                    Ok(())
                } else {
                    return Err(Error::AddIdentityBlock(format!(
                        "First Block validation error: block id: {}",
                        block.id
                    )));
                }
            }
            Ok(latest_block) => {
                // if there is a latest block, ensure it's a valid follow-up block
                if !block.validate_with_previous(&latest_block) {
                    return Err(Error::AddIdentityBlock(format!(
                        "Block validation error: block id: {}, latest block id: {}",
                        block.id, latest_block.id
                    )));
                }
                let _: Option<IdentityBlockDb> =
                    self.db.create(Self::TABLE).content(entity).await?;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityBlockDb {
    pub block_id: u64,
    pub hash: String,
    pub previous_hash: String,
    pub signature: String,
    pub timestamp: i64,
    pub public_key: String,
    pub data: String,
    pub op_code: IdentityOpCode,
}

impl From<IdentityBlockDb> for IdentityBlock {
    fn from(value: IdentityBlockDb) -> Self {
        Self {
            id: value.block_id,
            hash: value.hash,
            timestamp: value.timestamp,
            data: value.data,
            public_key: value.public_key,
            previous_hash: value.previous_hash,
            signature: value.signature,
            op_code: value.op_code,
        }
    }
}

impl From<&IdentityBlock> for IdentityBlockDb {
    fn from(value: &IdentityBlock) -> Self {
        Self {
            block_id: value.id,
            hash: value.hash.clone(),
            previous_hash: value.previous_hash.clone(),
            signature: value.signature.clone(),
            timestamp: value.timestamp,
            public_key: value.public_key.clone(),
            data: value.data.clone(),
            op_code: value.op_code.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        blockchain::identity::IdentityUpdateBlockData, persistence::db::get_memory_db,
        service::identity_service::Identity, tests::test::TEST_PUB_KEY, util::BcrKeys,
    };

    async fn get_store() -> SurrealIdentityChainStore {
        let mem_db = get_memory_db("test", "identity_chain")
            .await
            .expect("could not create get_memory_db");
        SurrealIdentityChainStore::new(mem_db)
    }

    #[tokio::test]
    async fn test_add_block() {
        let store = get_store().await;
        let block = IdentityBlock::create_block_for_create(
            1,
            "genesis hash".to_string(),
            &Identity::new_empty().into(),
            &BcrKeys::new(),
            TEST_PUB_KEY,
            1731593928,
        )
        .unwrap();
        store.add_block(&block).await.unwrap();
        let last_block = store.get_latest_block().await;
        assert!(last_block.is_ok());
        assert_eq!(last_block.as_ref().unwrap().id, 1);

        let block2 = IdentityBlock::create_block_for_update(
            &block,
            &IdentityUpdateBlockData {
                name: None,
                company: None,
                email: None,
                postal_address: None,
            },
            &BcrKeys::new(),
            TEST_PUB_KEY,
            1731593928,
        )
        .unwrap();
        store.add_block(&block2).await.unwrap();
        let last_block = store.get_latest_block().await;
        assert!(last_block.is_ok());
        assert_eq!(last_block.as_ref().unwrap().id, 2);
    }
}
