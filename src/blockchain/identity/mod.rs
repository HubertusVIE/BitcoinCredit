use super::Result;
use super::{calculate_hash, Block, Blockchain};
use crate::service::identity_service::Identity;
use crate::util::{crypto, rsa, BcrKeys};
use borsh::to_vec;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum IdentityOpCode {
    Create,
    Update,
    SignPersonBill,
    CreateCompany,
    AddSignatory,
    RemoveSignatory,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct IdentityBlock {
    pub id: u64,
    pub hash: String,
    pub timestamp: i64,
    pub data: String,
    pub public_key: String,
    pub previous_hash: String,
    pub signature: String,
    pub op_code: IdentityOpCode,
}

impl Block for IdentityBlock {
    type OpCode = IdentityOpCode;

    fn id(&self) -> u64 {
        self.id
    }

    fn timestamp(&self) -> i64 {
        self.timestamp
    }

    fn op_code(&self) -> &Self::OpCode {
        &self.op_code
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
}

impl IdentityBlock {
    pub fn new(
        id: u64,
        previous_hash: String,
        data: String,
        op_code: IdentityOpCode,
        keys: &BcrKeys,
        timestamp: i64,
    ) -> Result<Self> {
        let hash = calculate_hash(
            &id,
            &previous_hash,
            &data,
            &timestamp,
            &keys.get_public_key(),
            &op_code,
        );
        let signature = crypto::signature(&hash, &keys.get_private_key_string())?;

        Ok(Self {
            id,
            hash,
            timestamp,
            previous_hash,
            signature,
            public_key: keys.get_public_key(),
            data,
            op_code,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IdentityBlockchain {
    blocks: Vec<IdentityBlock>,
}

impl Blockchain for IdentityBlockchain {
    type Block = IdentityBlock;

    fn blocks(&self) -> &Vec<Self::Block> {
        &self.blocks
    }

    fn blocks_mut(&mut self) -> &mut Vec<Self::Block> {
        &mut self.blocks
    }
}

impl IdentityBlockchain {
    /// Creates a new identity chain, encrypting the identity with the public rsa key
    pub fn new(identity: &Identity, keys: &BcrKeys, timestamp: i64) -> Result<Self> {
        let identity_bytes = to_vec(&identity)?;
        let genesis_hash = hex::encode(&identity_bytes);

        let encrypted_data = hex::encode(rsa::encrypt_bytes_with_public_key(
            &identity_bytes,
            &identity.public_key_pem,
        )?);

        let first_block = IdentityBlock::new(
            1,
            genesis_hash,
            encrypted_data,
            IdentityOpCode::Create,
            keys,
            timestamp,
        )?;

        Ok(Self {
            blocks: vec![first_block],
        })
    }

    /// Creates a blockchain from a list of blocks, ensuring that the resulting chain is valid
    pub fn create_valid_chain_from_blocks(blocks: Vec<IdentityBlock>) -> Result<Self> {
        if blocks.is_empty() {
            return Err(super::Error::BlockchainInvalid);
        }

        if let Some(first_block) = blocks.first() {
            if first_block.id != 1 {
                return Err(super::Error::BlockchainInvalid);
            }
        }

        let created = Self { blocks };

        if !created.is_chain_valid() {
            return Err(super::Error::BlockchainInvalid);
        }
        Ok(created)
    }
}
