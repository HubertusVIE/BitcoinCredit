use super::Result;
use crate::blockchain::identity::{IdentityBlock, IdentityBlockchain};
use async_trait::async_trait;

#[cfg(test)]
use mockall::automock;

#[cfg_attr(test, automock)]
#[async_trait]
pub trait IdentityChainStoreApi: Send + Sync {
    /// Gets the whole chain
    #[allow(dead_code)]
    async fn get_chain(&self) -> Result<IdentityBlockchain>;
    /// Gets the latest block of the chain
    async fn get_latest_block(&self) -> Result<IdentityBlock>;
    /// Adds the block to the chain
    async fn add_block(&self, block: &IdentityBlock) -> Result<()>;
}
