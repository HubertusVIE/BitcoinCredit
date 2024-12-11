use thiserror::Error;

use crate::external;
use crate::util::rsa;
use log::{error, warn};
use std::string::FromUtf8Error;

pub mod bill;

/// Generic result type
pub type Result<T> = std::result::Result<T, Error>;

/// Generic error type
#[derive(Debug, Error)]
pub enum Error {
    /// Errors from io handling, or binary serialization/deserialization
    #[error("io error {0}")]
    Io(#[from] std::io::Error),

    /// If a whole chain is not valid
    #[error("Blockchain is invalid")]
    BlockchainInvalid,

    /// Errors stemming from json deserialization. Most of the time this is a
    #[error("unable to serialize/deserialize to/from JSON {0}")]
    Json(#[from] serde_json::Error),

    /// Errors stemming from cryptography, such as converting keys, encryption and decryption
    #[error("Cryptography error: {0}")]
    Cryptography(#[from] rsa::Error),

    /// Errors stemming from decoding
    #[error("Decode error: {0}")]
    Decode(#[from] hex::FromHexError),

    /// Errors stemming from converting from utf-8 strings
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] FromUtf8Error),

    /// Errors stemming from dealing with invalid block data, e.g. if within an Endorse block,
    /// there is no endorsee
    #[error("Invalid block data error: {0}")]
    InvalidBlockdata(String),

    /// all errors originating from external APIs
    #[error("External API error: {0}")]
    ExternalApi(#[from] external::Error),
}

#[allow(dead_code)]
trait Block {
    type OpCode: PartialEq + Clone;

    fn id(&self) -> u64;
    fn timestamp(&self) -> i64;
    fn op_code(&self) -> Self::OpCode;
    fn hash(&self) -> &str;
    fn previous_hash(&self) -> &str;
    fn data(&self) -> &Vec<u8>;
    fn signature(&self) -> &Vec<u8>;
    fn public_key(&self) -> &Vec<u8>;

    fn validate_hash(&self) -> bool;
    fn verify(&self) -> bool;

    fn validate_with_previous(&self, previous_block: &Self) -> bool {
        if self.previous_hash() != previous_block.hash() {
            warn!("block with id: {} has wrong previous hash", self.id());
            return false;
        } else if self.id() != previous_block.id() + 1 {
            warn!(
                "block with id: {} is not the next block after the latest: {}",
                self.id(),
                previous_block.id()
            );
            return false;
        } else if !self.validate_hash() {
            warn!("block with id: {} has invalid hash", self.id());
            return false;
        } else if !self.verify() {
            warn!("block with id: {} has invalid signature", self.id());
            return false;
        }
        true
    }
}

#[allow(dead_code)]
trait Blockchain {
    type Block: Block + Clone;

    fn blocks(&self) -> &Vec<Self::Block>;

    fn blocks_mut(&mut self) -> &mut Vec<Self::Block>;

    fn get_block_with_op_code(&self, op_code: <Self::Block as Block>::OpCode) -> &Self::Block {
        self.blocks()
            .iter()
            .filter(|block| block.op_code() == op_code)
            .last()
            .unwrap_or_else(|| self.get_first_block())
    }

    fn is_chain_valid(&self) -> bool {
        let blocks = self.blocks();
        for i in 0..blocks.len() {
            if i == 0 {
                continue;
            }
            let first = &blocks[i - 1];
            let second = &blocks[i];
            if !second.validate_with_previous(first) {
                return false;
            }
        }
        true
    }

    fn try_add_block(&mut self, block: Self::Block) -> bool {
        let latest_block = self.get_latest_block();
        if block.validate_with_previous(latest_block) {
            self.blocks_mut().push(block);
            true
        } else {
            error!("could not add block - invalid");
            false
        }
    }

    fn get_latest_block(&self) -> &Self::Block {
        self.blocks().last().expect("there is at least one block")
    }

    fn get_first_block(&self) -> &Self::Block {
        self.blocks().first().expect("there is at least one block")
    }

    fn get_last_version_block_with_operation_code(
        &self,
        op_code: <Self::Block as Block>::OpCode,
    ) -> &Self::Block {
        self.blocks()
            .iter()
            .filter(|block| block.op_code() == op_code)
            .last()
            .unwrap_or_else(|| self.get_first_block())
    }

    fn block_with_operation_code_exists(&self, op_code: <Self::Block as Block>::OpCode) -> bool {
        self.blocks().iter().any(|b| b.op_code() == op_code)
    }

    fn get_block_by_id(&self, id: u64) -> Self::Block {
        self.blocks()
            .iter()
            .find(|b| b.id() == id)
            .cloned()
            .unwrap_or_else(|| self.get_first_block().clone())
    }
}
