use super::bill::BillOpCode;
use super::Result;
use super::{calculate_hash, Block, Blockchain};
use crate::service::identity_service::Identity;
use crate::util::{self, crypto, rsa, BcrKeys};
use borsh::to_vec;
use borsh_derive::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

#[derive(BorshSerialize, Serialize, Deserialize, Debug, Clone, PartialEq)]
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

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct IdentityCreateBlockData {
    pub name: String,
    pub company: String,
    pub date_of_birth: String,
    pub city_of_birth: String,
    pub country_of_birth: String,
    pub email: String,
    pub postal_address: String,
    pub nostr_relay: Option<String>,
}

impl From<Identity> for IdentityCreateBlockData {
    fn from(value: Identity) -> Self {
        Self {
            name: value.name,
            company: value.company,
            date_of_birth: value.date_of_birth,
            city_of_birth: value.city_of_birth,
            country_of_birth: value.country_of_birth,
            email: value.email,
            postal_address: value.postal_address,
            nostr_relay: value.nostr_relay,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct IdentityUpdateBlockData {
    pub name: Option<String>,
    pub company: Option<String>,
    pub email: Option<String>,
    pub postal_address: Option<String>,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct IdentitySignPersonBillBlockData {
    pub bill_id: String,
    pub block_id: u64,
    pub block_hash: String,
    pub operation: BillOpCode,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct IdentityCreateCompanyBlockData {
    pub company_id: String,
    pub block_hash: String,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct IdentityAddSignatoryBlockData {
    pub company_id: String,
    pub block_id: u64,
    pub block_hash: String,
    pub signatory: String,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct IdentityRemoveSignatoryBlockData {
    pub company_id: String,
    pub block_id: u64,
    pub block_hash: String,
    pub signatory: String,
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
    fn new(
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
        )?;
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

    pub fn create_block_for_create(
        id: u64,
        previous_hash: String,
        identity: &IdentityCreateBlockData,
        keys: &BcrKeys,
        rsa_public_key_pem: &str,
        timestamp: i64,
    ) -> Result<Self> {
        let identity_bytes = to_vec(identity)?;

        let encrypted_data = util::base58_encode(&rsa::encrypt_bytes_with_public_key(
            &identity_bytes,
            rsa_public_key_pem,
        )?);

        Self::new(
            id,
            previous_hash,
            encrypted_data,
            IdentityOpCode::Create,
            keys,
            timestamp,
        )
    }

    pub fn create_block_for_update(
        previous_block: &Self,
        data: &IdentityUpdateBlockData,
        keys: &BcrKeys,
        rsa_public_key_pem: &str,
        timestamp: i64,
    ) -> Result<Self> {
        let block = Self::encrypt_data_create_block_and_validate(
            previous_block,
            data,
            keys,
            rsa_public_key_pem,
            timestamp,
            IdentityOpCode::Update,
        )?;
        Ok(block)
    }

    pub fn create_block_for_sign_person_bill(
        previous_block: &Self,
        data: &IdentitySignPersonBillBlockData,
        keys: &BcrKeys,
        rsa_public_key_pem: &str,
        timestamp: i64,
    ) -> Result<Self> {
        let block = Self::encrypt_data_create_block_and_validate(
            previous_block,
            data,
            keys,
            rsa_public_key_pem,
            timestamp,
            IdentityOpCode::SignPersonBill,
        )?;
        Ok(block)
    }

    pub fn create_block_for_create_company(
        previous_block: &Self,
        data: &IdentityCreateCompanyBlockData,
        keys: &BcrKeys,
        rsa_public_key_pem: &str,
        timestamp: i64,
    ) -> Result<Self> {
        let block = Self::encrypt_data_create_block_and_validate(
            previous_block,
            data,
            keys,
            rsa_public_key_pem,
            timestamp,
            IdentityOpCode::CreateCompany,
        )?;
        Ok(block)
    }

    pub fn create_block_for_add_signatory(
        previous_block: &Self,
        data: &IdentityAddSignatoryBlockData,
        keys: &BcrKeys,
        rsa_public_key_pem: &str,
        timestamp: i64,
    ) -> Result<Self> {
        let block = Self::encrypt_data_create_block_and_validate(
            previous_block,
            data,
            keys,
            rsa_public_key_pem,
            timestamp,
            IdentityOpCode::AddSignatory,
        )?;
        Ok(block)
    }

    pub fn create_block_for_remove_signatory(
        previous_block: &Self,
        data: &IdentityRemoveSignatoryBlockData,
        keys: &BcrKeys,
        rsa_public_key_pem: &str,
        timestamp: i64,
    ) -> Result<Self> {
        let block = Self::encrypt_data_create_block_and_validate(
            previous_block,
            data,
            keys,
            rsa_public_key_pem,
            timestamp,
            IdentityOpCode::RemoveSignatory,
        )?;
        Ok(block)
    }

    fn encrypt_data_create_block_and_validate<T: borsh::BorshSerialize>(
        previous_block: &Self,
        data: &T,
        keys: &BcrKeys,
        rsa_public_key_pem: &str,
        timestamp: i64,
        op_code: IdentityOpCode,
    ) -> Result<Self> {
        let bytes = to_vec(&data)?;

        let encrypted_data = util::base58_encode(&rsa::encrypt_bytes_with_public_key(
            &bytes,
            rsa_public_key_pem,
        )?);

        let new_block = Self::new(
            previous_block.id + 1,
            previous_block.hash.clone(),
            encrypted_data,
            op_code,
            keys,
            timestamp,
        )?;

        if !new_block.validate_with_previous(previous_block) {
            return Err(super::Error::BlockInvalid);
        }
        Ok(new_block)
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
    pub fn new(
        identity: &IdentityCreateBlockData,
        node_id: &str,
        keys: &BcrKeys,
        rsa_public_key_pem: &str,
        timestamp: i64,
    ) -> Result<Self> {
        let genesis_hash = util::base58_encode(node_id.as_bytes());

        let first_block = IdentityBlock::create_block_for_create(
            1,
            genesis_hash,
            identity,
            keys,
            rsa_public_key_pem,
            timestamp,
        )?;

        Ok(Self {
            blocks: vec![first_block],
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tests::test::TEST_PUB_KEY;
    use libp2p::PeerId;

    #[test]
    fn create_and_check_validity() {
        let mut identity = Identity::new_empty();
        identity.public_key_pem = TEST_PUB_KEY.to_string();

        let chain = IdentityBlockchain::new(
            &identity.into(),
            &PeerId::random().to_string(),
            &BcrKeys::new(),
            TEST_PUB_KEY,
            1731593928,
        );
        assert!(chain.is_ok());
        assert!(chain.as_ref().unwrap().is_chain_valid());
    }
}
