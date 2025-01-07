use super::bill::BillOpCode;
use super::Result;
use super::{calculate_hash, Block, Blockchain};
use crate::service::company_service::{CompanyKeys, CompanyToReturn};
use crate::util::{self, crypto, rsa, BcrKeys};
use crate::web::data::File;
use borsh::to_vec;
use borsh_derive::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

#[derive(BorshSerialize, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum CompanyOpCode {
    Create,
    Update,
    AddSignatory,
    RemoveSignatory,
    SignCompanyBill,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SignatoryType {
    Solo,
}

/// Structure for the block data of a company block
///
/// - `data` contains the actual data of the block, encrypted using the company's RSA pub key
/// - `key` is optional and if set, contains the company private keys encrypted by an identity RSA
///   pub key (e.g. for CreateCompany the creator's and AddSignatory the signatory's)
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct CompanyBlockData {
    data: String,
    key: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CompanyBlock {
    pub company_id: String,
    pub id: u64,
    pub hash: String,
    pub timestamp: u64,
    pub data: String,
    pub public_key: String,
    pub previous_hash: String,
    pub signature: String,
    pub op_code: CompanyOpCode,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct CompanyCreateBlockData {
    pub id: String,
    pub name: String,
    pub country_of_registration: String,
    pub city_of_registration: String,
    pub postal_address: String,
    pub email: String,
    pub registration_number: String,
    pub registration_date: String,
    pub proof_of_registration_file: Option<File>,
    pub logo_file: Option<File>,
    pub signatories: Vec<String>,
    pub public_key: String,
    pub rsa_public_key: String,
}

impl From<CompanyToReturn> for CompanyCreateBlockData {
    fn from(value: CompanyToReturn) -> Self {
        Self {
            id: value.id,
            name: value.name,
            country_of_registration: value.country_of_registration,
            city_of_registration: value.city_of_registration,
            postal_address: value.postal_address,
            email: value.email,
            registration_number: value.registration_number,
            registration_date: value.registration_date,
            proof_of_registration_file: value.proof_of_registration_file,
            logo_file: value.logo_file,
            signatories: value.signatories,
            public_key: value.public_key,
            rsa_public_key: value.rsa_public_key,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct CompanyUpdateBlockData {
    pub name: Option<String>,
    pub email: Option<String>,
    pub postal_address: Option<String>,
    pub logo_file_upload_id: Option<String>,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct CompanySignCompanyBillBlockData {
    pub bill_id: String,
    pub block_id: u64,
    pub block_hash: String,
    pub operation: BillOpCode,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct CompanyAddSignatoryBlockData {
    pub signatory: String,
    pub t: SignatoryType,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct CompanyRemoveSignatoryBlockData {
    pub signatory: String,
}

impl Block for CompanyBlock {
    type OpCode = CompanyOpCode;

    fn id(&self) -> u64 {
        self.id
    }

    fn timestamp(&self) -> u64 {
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

impl CompanyBlock {
    fn new(
        company_id: String,
        id: u64,
        previous_hash: String,
        data: String,
        op_code: CompanyOpCode,
        _identity_keys: &BcrKeys,
        company_keys: &CompanyKeys,
        timestamp: u64,
    ) -> Result<Self> {
        // TODO: calculate aggregated public key from identity and company key
        let hash = calculate_hash(
            &id,
            &previous_hash,
            &data,
            &timestamp,
            &company_keys.public_key, // TODO: use aggregated key
            &op_code,
        )?;
        // TODO: use aggregated signature from identity and company key
        let signature = crypto::signature(&hash, &company_keys.private_key)?;

        Ok(Self {
            company_id,
            id,
            hash,
            timestamp,
            previous_hash,
            signature,
            public_key: company_keys.public_key.clone(), // TODO: use aggregated key
            data,
            op_code,
        })
    }

    pub fn create_block_for_create(
        company_id: String,
        id: u64,
        genesis_hash: String,
        company: &CompanyCreateBlockData,
        identity_keys: &BcrKeys,
        company_keys: &CompanyKeys,
        rsa_public_key_pem: &str, // creator's rsa key
        timestamp: u64,
    ) -> Result<Self> {
        let company_bytes = to_vec(company)?;
        // encrypt data using company rsa key
        let encrypted_data = util::base58_encode(&rsa::encrypt_bytes_with_public_key(
            &company_bytes,
            &company_keys.rsa_public_key,
        )?);

        let keys_bytes = to_vec(&company_keys)?;
        // encrypt company keys using creator's identity rsa key
        let encrypted_keys = util::base58_encode(&rsa::encrypt_bytes_with_public_key(
            &keys_bytes,
            rsa_public_key_pem,
        )?);

        let data = CompanyBlockData {
            data: encrypted_data,
            key: Some(encrypted_keys),
        };
        let serialized_and_hashed_data = util::base58_encode(&to_vec(&data)?);

        Self::new(
            company_id.to_owned(),
            id,
            genesis_hash,
            serialized_and_hashed_data,
            CompanyOpCode::Create,
            identity_keys,
            company_keys,
            timestamp,
        )
    }

    pub fn create_block_for_update(
        company_id: String,
        previous_block: &Self,
        data: &CompanyUpdateBlockData,
        identity_keys: &BcrKeys,
        company_keys: &CompanyKeys,
        timestamp: u64,
    ) -> Result<Self> {
        let block = Self::encrypt_data_create_block_and_validate(
            company_id,
            previous_block,
            data,
            identity_keys,
            company_keys,
            None,
            timestamp,
            CompanyOpCode::Update,
        )?;
        Ok(block)
    }

    #[allow(dead_code)]
    pub fn create_block_for_sign_company_bill(
        company_id: String,
        previous_block: &Self,
        data: &CompanySignCompanyBillBlockData,
        identity_keys: &BcrKeys,
        company_keys: &CompanyKeys,
        timestamp: u64,
    ) -> Result<Self> {
        let block = Self::encrypt_data_create_block_and_validate(
            company_id,
            previous_block,
            data,
            identity_keys,
            company_keys,
            None,
            timestamp,
            CompanyOpCode::SignCompanyBill,
        )?;
        Ok(block)
    }

    pub fn create_block_for_add_signatory(
        company_id: String,
        previous_block: &Self,
        data: &CompanyAddSignatoryBlockData,
        identity_keys: &BcrKeys,
        company_keys: &CompanyKeys,
        rsa_public_key_pem: &str, // the signatory's public rsa key
        timestamp: u64,
    ) -> Result<Self> {
        let block = Self::encrypt_data_create_block_and_validate(
            company_id,
            previous_block,
            data,
            identity_keys,
            company_keys,
            Some(rsa_public_key_pem),
            timestamp,
            CompanyOpCode::AddSignatory,
        )?;
        Ok(block)
    }

    pub fn create_block_for_remove_signatory(
        company_id: String,
        previous_block: &Self,
        data: &CompanyRemoveSignatoryBlockData,
        identity_keys: &BcrKeys,
        company_keys: &CompanyKeys,
        timestamp: u64,
    ) -> Result<Self> {
        let block = Self::encrypt_data_create_block_and_validate(
            company_id,
            previous_block,
            data,
            identity_keys,
            company_keys,
            None,
            timestamp,
            CompanyOpCode::RemoveSignatory,
        )?;
        Ok(block)
    }

    fn encrypt_data_create_block_and_validate<T: borsh::BorshSerialize>(
        company_id: String,
        previous_block: &Self,
        data: &T,
        identity_keys: &BcrKeys,
        company_keys: &CompanyKeys,
        rsa_public_key_pem: Option<&str>,
        timestamp: u64,
        op_code: CompanyOpCode,
    ) -> Result<Self> {
        let bytes = to_vec(&data)?;
        // encrypt data using the company rsa key
        let encrypted_data = util::base58_encode(&rsa::encrypt_bytes_with_public_key(
            &bytes,
            &company_keys.rsa_public_key,
        )?);

        let mut keys = None;

        // in case there are keys to encrypt, encrypt them using the receiver's identity rsa pub
        // key
        if op_code == CompanyOpCode::AddSignatory {
            if let Some(signatory_rsa_public_key) = rsa_public_key_pem {
                let keys_bytes = to_vec(&company_keys)?;
                let encrypted_keys = util::base58_encode(&rsa::encrypt_bytes_with_public_key(
                    &keys_bytes,
                    signatory_rsa_public_key,
                )?);
                keys = Some(encrypted_keys);
            }
        }

        let data = CompanyBlockData {
            data: encrypted_data,
            key: keys,
        };
        let serialized_and_hashed_data = util::base58_encode(&to_vec(&data)?);

        let new_block = Self::new(
            company_id,
            previous_block.id + 1,
            previous_block.hash.clone(),
            serialized_and_hashed_data,
            op_code,
            identity_keys,
            company_keys,
            timestamp,
        )?;

        if !new_block.validate_with_previous(previous_block) {
            return Err(super::Error::BlockInvalid);
        }
        Ok(new_block)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CompanyBlockchain {
    blocks: Vec<CompanyBlock>,
}

impl Blockchain for CompanyBlockchain {
    type Block = CompanyBlock;

    fn blocks(&self) -> &Vec<Self::Block> {
        &self.blocks
    }

    fn blocks_mut(&mut self) -> &mut Vec<Self::Block> {
        &mut self.blocks
    }
}

impl CompanyBlockchain {
    /// Creates a new company chain
    pub fn new(
        company: &CompanyCreateBlockData,
        node_id: &str,
        identity_keys: &BcrKeys,
        company_keys: &CompanyKeys,
        rsa_public_key_pem: &str,
        timestamp: u64,
    ) -> Result<Self> {
        let genesis_hash = util::base58_encode(node_id.as_bytes());

        let first_block = CompanyBlock::create_block_for_create(
            company.id.clone(),
            1,
            genesis_hash,
            company,
            identity_keys,
            company_keys,
            rsa_public_key_pem,
            timestamp,
        )?;

        Ok(Self {
            blocks: vec![first_block],
        })
    }

    /// Creates a company chain from a vec of blocks
    pub fn new_from_blocks(blocks_to_add: Vec<CompanyBlock>) -> Result<Self> {
        if blocks_to_add.is_empty() {
            return Err(super::Error::BlockchainInvalid);
        }

        let first = blocks_to_add
            .first()
            .expect("checked above that there is one block");
        if !first.verify() || !first.validate_hash() {
            return Err(super::Error::BlockchainInvalid);
        }

        let chain = Self {
            blocks: blocks_to_add,
        };

        if !chain.is_chain_valid() {
            return Err(super::Error::BlockchainInvalid);
        }

        Ok(chain)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        service::company_service::{test::get_baseline_company_data, CompanyToReturn},
        tests::test::TEST_PUB_KEY,
    };
    use libp2p::PeerId;

    #[test]
    fn create_and_check_validity() {
        let (id, (company, company_keys)) = get_baseline_company_data();
        let to_return = CompanyToReturn::from(id, company, company_keys.clone());

        let chain = CompanyBlockchain::new(
            &CompanyCreateBlockData::from(to_return),
            &PeerId::random().to_string(),
            &BcrKeys::new(),
            &company_keys,
            TEST_PUB_KEY,
            1731593928,
        );
        assert!(chain.is_ok());
        assert!(chain.as_ref().unwrap().is_chain_valid());
    }

    #[test]
    fn multi_block() {
        let (id, (company, company_keys)) = get_baseline_company_data();
        let to_return = CompanyToReturn::from(id.clone(), company, company_keys.clone());
        let identity_keys = BcrKeys::new();

        let chain = CompanyBlockchain::new(
            &CompanyCreateBlockData::from(to_return),
            &PeerId::random().to_string(),
            &identity_keys,
            &company_keys,
            TEST_PUB_KEY,
            1731593928,
        );
        assert!(chain.is_ok());
        assert!(chain.as_ref().unwrap().is_chain_valid());

        let mut chain = chain.unwrap();
        let update_block = CompanyBlock::create_block_for_update(
            id.to_owned(),
            chain.get_latest_block(),
            &CompanyUpdateBlockData {
                name: Some("new_name".to_string()),
                email: None,
                postal_address: None,
                logo_file_upload_id: None,
            },
            &identity_keys,
            &company_keys,
            1731593929,
        );
        assert!(update_block.is_ok());
        chain.try_add_block(update_block.unwrap());

        let bill_block = CompanyBlock::create_block_for_sign_company_bill(
            id.to_owned(),
            chain.get_latest_block(),
            &CompanySignCompanyBillBlockData {
                bill_id: "some_id".to_string(),
                block_id: 1,
                block_hash: "some hash".to_string(),
                operation: BillOpCode::Issue,
            },
            &identity_keys,
            &company_keys,
            1731593930,
        );
        assert!(bill_block.is_ok());
        chain.try_add_block(bill_block.unwrap());

        let add_signatory_block = CompanyBlock::create_block_for_add_signatory(
            id.to_owned(),
            chain.get_latest_block(),
            &CompanyAddSignatoryBlockData {
                signatory: "some_signatory".to_string(),
                t: SignatoryType::Solo,
            },
            &identity_keys,
            &company_keys,
            TEST_PUB_KEY,
            1731593931,
        );
        assert!(add_signatory_block.is_ok());
        chain.try_add_block(add_signatory_block.unwrap());

        let remove_signatory_block = CompanyBlock::create_block_for_remove_signatory(
            id.to_owned(),
            chain.get_latest_block(),
            &CompanyRemoveSignatoryBlockData {
                signatory: "some_signatory".to_string(),
            },
            &identity_keys,
            &company_keys,
            1731593932,
        );
        assert!(remove_signatory_block.is_ok());
        chain.try_add_block(remove_signatory_block.unwrap());

        assert_eq!(chain.blocks().len(), 5);
        assert!(chain.is_chain_valid());

        let new_chain_from_empty_blocks = CompanyBlockchain::new_from_blocks(vec![]);
        assert!(new_chain_from_empty_blocks.is_err());

        let blocks = chain.blocks();
        let new_chain_from_blocks = CompanyBlockchain::new_from_blocks(blocks.to_owned());
        assert!(new_chain_from_blocks.is_ok());
        assert!(new_chain_from_blocks.as_ref().unwrap().is_chain_valid());

        let mut_blocks = chain.blocks_mut();
        mut_blocks[2].hash = "invalidhash".to_string();
        let new_chain_from_invalid_blocks =
            CompanyBlockchain::new_from_blocks(mut_blocks.to_owned());
        assert!(new_chain_from_invalid_blocks.is_err());
    }
}
