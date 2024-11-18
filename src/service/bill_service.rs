use super::build_validation_response;
use super::contact_service::IdentityPublicData;
use super::identity_service::IdentityWithAll;
use crate::bill::{read_bill_from_file, BillFile, BillKeys};
use crate::blockchain::{
    self, start_blockchain_for_new_bill, Block, Chain, GossipsubEvent, GossipsubEventId,
    OperationCode,
};
use crate::constants::{
    COMPOUNDING_INTEREST_RATE_ZERO, MAX_FILE_NAME_CHARACTERS, MAX_FILE_SIZE_BYTES, USEDNET,
    VALID_FILE_MIME_TYPES,
};
use crate::persistence::identity::IdentityStoreApi;
use crate::{bill::BitcreditBill, dht::Client, persistence::bill::BillStoreApi};
use crate::{external, persistence, util};
use async_trait::async_trait;
use chrono::Utc;
use log::{error, info};
use openssl::pkey::Private;
use openssl::rsa::Rsa;
use rocket::{http::Status, response::Responder};
use std::sync::Arc;
use thiserror::Error;

/// Generic result type
pub type Result<T> = std::result::Result<T, Error>;

/// Generic error type
#[derive(Debug, Error)]
pub enum Error {
    /// error returned if a bill was already accepted and is attempted to be accepted again
    #[error("Bill was already accepted")]
    BillAlreadyAccepted,

    /// error returned if the caller of an operation is not the drawee, but would have to be for it
    /// to be valid, e.g. accepting a  bill
    #[error("Caller is not drawee")]
    CallerIsNotDrawee,

    /// error returned if the caller of an operation is not the payee, or endorsee, but would have to be for it
    /// to be valid, e.g. requesting payment
    #[error("Caller is not payee, or endorsee")]
    CallerIsNotPayeeOrEndorsee,

    /// errors stemming from json deserialization
    #[error("unable to serialize/deserialize to/from JSON {0}")]
    Json(#[from] serde_json::Error),

    /// errors that stem from validation
    #[error("Validation Error: {0}")]
    Validation(String),

    /// errors that stem from interacting with a blockchain
    #[error("Blockchain error: {0}")]
    Blockchain(#[from] blockchain::Error),

    /// all errors originating from the persistence layer
    #[error("Persistence error: {0}")]
    Persistence(#[from] persistence::Error),

    #[error("External API error: {0}")]
    ExternalApi(#[from] external::Error),

    /// errors stemming from cryptography, such as converting keys
    #[error("Cryptography error: {0}")]
    Cryptography(String),
}
impl<'r, 'o: 'r> Responder<'r, 'o> for Error {
    fn respond_to(self, req: &rocket::Request) -> rocket::response::Result<'o> {
        match self {
            Error::BillAlreadyAccepted => Status::BadRequest.respond_to(req),
            Error::CallerIsNotDrawee => Status::BadRequest.respond_to(req),
            Error::CallerIsNotPayeeOrEndorsee => Status::BadRequest.respond_to(req),
            Error::Json(e) => {
                error!("{e}");
                Status::InternalServerError.respond_to(req)
            }
            Error::Persistence(e) => {
                error!("{e}");
                Status::InternalServerError.respond_to(req)
            }
            Error::ExternalApi(e) => {
                error!("{e}");
                Status::InternalServerError.respond_to(req)
            }
            Error::Validation(msg) => build_validation_response(msg),
            Error::Blockchain(e) => {
                error!("{e}");
                Status::InternalServerError.respond_to(req)
            }
            Error::Cryptography(e) => {
                error!("{e}");
                Status::InternalServerError.respond_to(req)
            }
        }
    }
}

#[async_trait]
pub trait BillServiceApi: Send + Sync {
    /// Gets the keys for a given bill
    async fn get_bill_keys(&self, bill_name: &str) -> Result<BillKeys>;

    /// opens and decrypts the attached file from the given bill
    async fn open_and_decrypt_attached_file(
        &self,
        bill_name: &str,
        file_name: &str,
        bill_private_key: &str,
    ) -> Result<Vec<u8>>;

    /// encrypts and saves the given uploaded file, returning the file name, as well as the hash of
    /// the unencrypted file
    async fn encrypt_and_save_uploaded_file(
        &self,
        file: &dyn util::file::UploadFileHandler,
        bill_name: &str,
        bill_public_key: &str,
    ) -> Result<BillFile>;

    /// validates the given uploaded file
    async fn validate_attached_file(&self, file: &dyn util::file::UploadFileHandler) -> Result<()>;

    /// issues a new bill
    async fn issue_new_bill(
        &self,
        bill_jurisdiction: String,
        place_of_drawing: String,
        amount_numbers: u64,
        place_of_payment: String,
        maturity_date: String,
        currency_code: String,
        drawer: IdentityWithAll,
        language: String,
        public_data_drawee: IdentityPublicData,
        public_data_payee: IdentityPublicData,
        files: Vec<&dyn util::file::UploadFileHandler>,
    ) -> Result<BitcreditBill>;

    /// propagates the given bill to the DHT
    async fn propagate_bill(
        &self,
        bill_name: &str,
        drawer_peer_id: &str,
        drawee_peer_id: &str,
        payee_peer_id: &str,
    ) -> Result<()>;

    /// accepts the given bill
    async fn accept_bill(&self, bill_name: &str) -> Result<()>;

    /// request pay for a bill
    async fn request_pay(&self, bill_name: &str, timestamp: i64) -> Result<Chain>;

    /// request acceptance for a bill
    async fn request_acceptance(&self, bill_name: &str, timestamp: i64) -> Result<Chain>;

    /// mint bitcredit bill
    async fn mint_bitcredit_bill(
        &self,
        bill_name: &str,
        mintnode: IdentityPublicData,
        timestamp: i64,
    ) -> Result<Chain>;

    /// sell bitcredit bill
    async fn sell_bitcredit_bill(
        &self,
        bill_name: &str,
        buyer: IdentityPublicData,
        timestamp: i64,
        amount_numbers: u64,
    ) -> Result<Chain>;

    /// endorse bitcredit bill
    async fn endorse_bitcredit_bill(
        &self,
        bill_name: &str,
        endorsee: IdentityPublicData,
        timestamp: i64,
    ) -> Result<Chain>;
}

/// The bill service is responsible for all bill-related logic and for syncing them with the dht data.
#[derive(Clone)]
pub struct BillService {
    client: Client,
    store: Arc<dyn BillStoreApi>,
    identity_store: Arc<dyn IdentityStoreApi>,
}

impl BillService {
    pub fn new(
        client: Client,
        store: Arc<dyn BillStoreApi>,
        identity_store: Arc<dyn IdentityStoreApi>,
    ) -> Self {
        Self {
            client,
            store,
            identity_store,
        }
    }

    fn get_data_for_new_block(
        &self,
        identity: &IdentityWithAll,
        prefix: &str,
        other_party: Option<(&IdentityPublicData, &str)>,
        postfix: &str,
    ) -> Result<String> {
        let identity_public =
            IdentityPublicData::new(identity.identity.clone(), identity.peer_id.to_string());
        let caller_identity_bytes = serde_json::to_vec(&identity_public)?;
        match other_party {
            None => Ok(format!(
                "{prefix}{}{postfix}",
                &hex::encode(caller_identity_bytes)
            )),
            Some((other_identity, midfix)) => {
                let other_identity_bytes = serde_json::to_vec(&other_identity)?;
                Ok(format!(
                    "{prefix}{}{midfix}{}{postfix}",
                    &hex::encode(other_identity_bytes),
                    &hex::encode(caller_identity_bytes)
                ))
            }
        }
    }

    /// attempts to add a block for the given operation and with the given prefix to the chain,
    /// mutating the chain
    async fn add_block_for_operation(
        &self,
        bill_name: &str,
        blockchain: &mut Chain,
        timestamp: i64,
        operation_code: OperationCode,
        identity: IdentityWithAll,
        data_for_new_block: String,
    ) -> Result<()> {
        let last_block = blockchain.get_latest_block();

        let keys = self.store.read_bill_keys_from_file(bill_name).await?;
        let key: Rsa<Private> = Rsa::private_key_from_pem(keys.private_key_pem.as_bytes())
            .map_err(|_| Error::Validation(String::from("invalid bill key")))?;

        let data_for_new_block_in_bytes = data_for_new_block.as_bytes();
        let data_for_new_block_encrypted =
            util::rsa::encrypt_bytes(data_for_new_block_in_bytes, &key);
        let data_for_new_block_encrypted_in_string_format =
            hex::encode(data_for_new_block_encrypted);

        let new_block = Block::new(
            last_block.id + 1,
            last_block.hash.clone(),
            data_for_new_block_encrypted_in_string_format,
            bill_name.to_owned(),
            identity.identity.public_key_pem,
            operation_code,
            identity.identity.private_key_pem,
            timestamp,
        );

        let try_add_block = blockchain.try_add_block(new_block);
        if try_add_block && blockchain.is_chain_valid() {
            self.store
                .write_blockchain_to_file(bill_name, blockchain.to_pretty_printed_json()?)
                .await?;
            Ok(())
        } else {
            Err(Error::Blockchain(blockchain::Error::BlockchainInvalid))
        }
    }
}

#[async_trait]
impl BillServiceApi for BillService {
    async fn get_bill_keys(&self, bill_name: &str) -> Result<BillKeys> {
        let keys = self.store.read_bill_keys_from_file(bill_name).await?;
        Ok(keys)
    }

    async fn open_and_decrypt_attached_file(
        &self,
        bill_name: &str,
        file_name: &str,
        bill_private_key: &str,
    ) -> Result<Vec<u8>> {
        let read_file = self.store.open_attached_file(bill_name, file_name).await?;
        let decrypted =
            util::rsa::decrypt_bytes_with_private_key(&read_file, bill_private_key.to_owned());
        Ok(decrypted)
    }

    async fn encrypt_and_save_uploaded_file(
        &self,
        file: &dyn util::file::UploadFileHandler,
        bill_name: &str,
        bill_public_key: &str,
    ) -> Result<BillFile> {
        let read_file = file.get_contents().await.map_err(persistence::Error::Io)?;
        let file_name = util::file::generate_unique_filename(
            &util::file::sanitize_filename(
                &file
                    .name()
                    .ok_or(Error::Validation(String::from("Invalid file name")))?,
            ),
            file.extension(),
        );

        let file_hash = util::sha256_hash(&read_file);
        let encrypted = util::rsa::encrypt_bytes_with_public_key(&read_file, bill_public_key);
        self.store
            .save_attached_file(&encrypted, bill_name, &file_name)
            .await?;
        info!("Saved file {file_name} with hash {file_hash} for bill {bill_name}");
        Ok(BillFile {
            name: file_name.to_owned(),
            hash: file_hash,
        })
    }

    async fn validate_attached_file(&self, file: &dyn util::file::UploadFileHandler) -> Result<()> {
        if file.len() > MAX_FILE_SIZE_BYTES as u64 {
            return Err(Error::Validation(format!(
                "Maximum file size is {} bytes",
                MAX_FILE_SIZE_BYTES
            )));
        }

        let name = match file.name() {
            Some(n) => n,
            None => {
                return Err(Error::Validation(String::from("File name needs to be set")));
            }
        };

        if name.is_empty() || name.len() > MAX_FILE_NAME_CHARACTERS {
            return Err(Error::Validation(format!(
                "File name needs to have between 1 and {} characters",
                MAX_FILE_NAME_CHARACTERS
            )));
        }

        let detected_type = match file.detect_content_type().await.map_err(|e| {
            error!("Could not detect content type for file {name}: {e}");
            Error::Validation(String::from("Could not detect content type for file"))
        })? {
            Some(t) => t,
            None => {
                return Err(Error::Validation(String::from(
                    "Unknown file type detected",
                )))
            }
        };

        if !VALID_FILE_MIME_TYPES.contains(&detected_type.as_str()) {
            return Err(Error::Validation(String::from(
                "Invalid file type detected",
            )));
        }
        Ok(())
    }

    async fn issue_new_bill(
        &self,
        bill_jurisdiction: String,
        place_of_drawing: String,
        amount_numbers: u64,
        place_of_payment: String,
        maturity_date: String,
        currency_code: String,
        drawer: IdentityWithAll,
        language: String,
        public_data_drawee: IdentityPublicData,
        public_data_payee: IdentityPublicData,
        files: Vec<&dyn util::file::UploadFileHandler>,
    ) -> Result<BitcreditBill> {
        let timestamp = external::time::TimeApi::get_atomic_time().await?.timestamp;
        let s = bitcoin::secp256k1::Secp256k1::new();
        let private_key = bitcoin::PrivateKey::new(
            s.generate_keypair(&mut bitcoin::secp256k1::rand::thread_rng())
                .0,
            USEDNET,
        );
        let public_key = private_key.public_key(&s);

        let bill_name = util::sha256_hash(&public_key.to_bytes());

        let private_key_bitcoin: String = private_key.to_string();
        let public_key_bitcoin: String = public_key.to_string();

        let rsa: Rsa<Private> = util::rsa::generation_rsa_key();
        let private_key_pem: String = util::rsa::pem_private_key_from_rsa(&rsa)
            .map_err(|e| Error::Cryptography(e.to_string()))?;
        let public_key_pem: String = util::rsa::pem_public_key_from_rsa(&rsa)
            .map_err(|e| Error::Cryptography(e.to_string()))?;
        self.store
            .write_bill_keys_to_file(
                bill_name.clone(),
                private_key_pem.clone(),
                public_key_pem.clone(),
            )
            .await?;

        let amount_letters: String = util::numbers_to_words::encode(&amount_numbers);

        let public_data_drawer =
            IdentityPublicData::new(drawer.identity.clone(), drawer.peer_id.to_string());

        let utc = Utc::now();
        let date_of_issue = utc.naive_local().date().to_string();

        let to_payee = public_data_drawer == public_data_payee;

        let mut bill_files: Vec<BillFile> = Vec::with_capacity(files.len());

        for file in files {
            bill_files.push(
                self.encrypt_and_save_uploaded_file(file, &bill_name, &public_key_pem)
                    .await?,
            );
        }

        let bill = BitcreditBill {
            name: bill_name,
            to_payee,
            bill_jurisdiction,
            timestamp_at_drawing: timestamp,
            place_of_drawing,
            currency_code,
            amount_numbers,
            amounts_letters: amount_letters,
            maturity_date,
            date_of_issue,
            compounding_interest_rate: COMPOUNDING_INTEREST_RATE_ZERO,
            type_of_interest_calculation: false,
            place_of_payment,
            public_key: public_key_bitcoin,
            private_key: private_key_bitcoin,
            language,
            drawee: public_data_drawee,
            drawer: public_data_drawer.clone(),
            payee: public_data_payee,
            endorsee: IdentityPublicData::new_empty(),
            files: bill_files,
        };

        start_blockchain_for_new_bill(
            &bill,
            OperationCode::Issue,
            public_data_drawer,
            drawer.identity.public_key_pem,
            drawer.identity.private_key_pem,
            private_key_pem,
            timestamp,
        );

        Ok(bill)
    }

    async fn propagate_bill(
        &self,
        bill_name: &str,
        drawer_peer_id: &str,
        drawee_peer_id: &str,
        payee_peer_id: &str,
    ) -> Result<()> {
        let mut client = self.client.clone();

        for node in [drawer_peer_id, drawee_peer_id, payee_peer_id] {
            if !node.is_empty() {
                info!("issue bill: add {} for node {}", bill_name, &node);
                client.add_bill_to_dht_for_node(bill_name, node).await;
            }
        }

        client.subscribe_to_topic(bill_name.to_owned()).await;

        client.put(bill_name).await;
        Ok(())
    }

    async fn accept_bill(&self, bill_name: &str) -> Result<()> {
        let my_peer_id = self.identity_store.get_peer_id().await?.to_string();
        let bill = read_bill_from_file(bill_name).await;

        let mut blockchain = Chain::read_chain_from_file(bill_name);
        let accepted = blockchain.exist_block_with_operation_code(OperationCode::Accept);

        if accepted {
            return Err(Error::BillAlreadyAccepted);
        }

        if !bill.drawee.peer_id.eq(&my_peer_id) {
            return Err(Error::CallerIsNotDrawee);
        }

        let timestamp = external::time::TimeApi::get_atomic_time().await?.timestamp;
        let identity = self.identity_store.get_full().await?;
        let data_for_new_block =
            self.get_data_for_new_block(&identity, "Accepted by ", None, "")?;
        self.add_block_for_operation(
            bill_name,
            &mut blockchain,
            timestamp,
            OperationCode::Accept,
            identity,
            data_for_new_block,
        )
        .await?;

        let mut client = self.client.clone();

        let block = blockchain.get_latest_block();
        let block_bytes = serde_json::to_vec(block)?;
        let event = GossipsubEvent::new(GossipsubEventId::Block, block_bytes);

        client
            .add_message_to_topic(event.to_byte_array(), bill_name.to_owned())
            .await;
        Ok(())
    }

    async fn request_pay(&self, bill_name: &str, timestamp: i64) -> Result<Chain> {
        let my_peer_id = self.identity_store.get_peer_id().await?.to_string();
        let bill = read_bill_from_file(bill_name).await;

        let mut blockchain = Chain::read_chain_from_file(bill_name);

        let exist_block_with_code_endorse =
            blockchain.exist_block_with_operation_code(OperationCode::Endorse);

        let exist_block_with_code_mint =
            blockchain.exist_block_with_operation_code(OperationCode::Mint);

        let exist_block_with_code_sell =
            blockchain.exist_block_with_operation_code(OperationCode::Sell);

        if !((my_peer_id.eq(&bill.payee.peer_id)
            && !exist_block_with_code_endorse
            && !exist_block_with_code_sell
            && !exist_block_with_code_mint)
            || (my_peer_id.eq(&bill.endorsee.peer_id)))
        {
            return Err(Error::CallerIsNotPayeeOrEndorsee);
        }

        let identity = self.identity_store.get_full().await?;
        let data_for_new_block =
            self.get_data_for_new_block(&identity, "Requested to pay by ", None, "")?;
        self.add_block_for_operation(
            bill_name,
            &mut blockchain,
            timestamp,
            OperationCode::RequestToPay,
            identity,
            data_for_new_block,
        )
        .await?;
        Ok(blockchain)
    }

    async fn request_acceptance(&self, bill_name: &str, timestamp: i64) -> Result<Chain> {
        let my_peer_id = self.identity_store.get_peer_id().await?.to_string();
        let bill = read_bill_from_file(bill_name).await;

        let mut blockchain = Chain::read_chain_from_file(bill_name);

        let exist_block_with_code_endorse =
            blockchain.exist_block_with_operation_code(OperationCode::Endorse);

        let exist_block_with_code_sell =
            blockchain.exist_block_with_operation_code(OperationCode::Sell);

        let exist_block_with_code_mint =
            blockchain.exist_block_with_operation_code(OperationCode::Mint);

        if !((my_peer_id.eq(&bill.payee.peer_id)
            && !exist_block_with_code_endorse
            && !exist_block_with_code_sell
            && !exist_block_with_code_mint)
            || (my_peer_id.eq(&bill.endorsee.peer_id)))
        {
            return Err(Error::CallerIsNotPayeeOrEndorsee);
        }
        let identity = self.identity_store.get_full().await?;
        let data_for_new_block =
            self.get_data_for_new_block(&identity, "Requested to accept by ", None, "")?;
        self.add_block_for_operation(
            bill_name,
            &mut blockchain,
            timestamp,
            OperationCode::RequestToAccept,
            identity,
            data_for_new_block,
        )
        .await?;
        Ok(blockchain)
    }

    async fn mint_bitcredit_bill(
        &self,
        bill_name: &str,
        mintnode: IdentityPublicData,
        timestamp: i64,
    ) -> Result<Chain> {
        let my_peer_id = self.identity_store.get_peer_id().await?.to_string();
        let bill = read_bill_from_file(bill_name).await;

        let mut blockchain = Chain::read_chain_from_file(bill_name);

        let exist_block_with_code_endorse =
            blockchain.exist_block_with_operation_code(OperationCode::Endorse);

        let exist_block_with_code_mint =
            blockchain.exist_block_with_operation_code(OperationCode::Mint);

        let exist_block_with_code_sell =
            blockchain.exist_block_with_operation_code(OperationCode::Sell);

        if !((my_peer_id.eq(&bill.payee.peer_id)
            && !exist_block_with_code_endorse
            && !exist_block_with_code_sell
            && !exist_block_with_code_mint)
            || (my_peer_id.eq(&bill.endorsee.peer_id)))
        {
            return Err(Error::CallerIsNotPayeeOrEndorsee);
        }

        let identity = self.identity_store.get_full().await?;
        let data_for_new_block = self.get_data_for_new_block(
            &identity,
            "Endorsed to ",
            Some((&mintnode, " endorsed by ")),
            "",
        )?;
        self.add_block_for_operation(
            bill_name,
            &mut blockchain,
            timestamp,
            OperationCode::Mint,
            identity,
            data_for_new_block,
        )
        .await?;
        Ok(blockchain)
    }

    async fn sell_bitcredit_bill(
        &self,
        bill_name: &str,
        buyer: IdentityPublicData,
        timestamp: i64,
        amount_numbers: u64,
    ) -> Result<Chain> {
        let my_peer_id = self.identity_store.get_peer_id().await?.to_string();
        let bill = read_bill_from_file(bill_name).await;

        let mut blockchain = Chain::read_chain_from_file(bill_name);

        let exist_block_with_code_endorse =
            blockchain.exist_block_with_operation_code(OperationCode::Endorse);

        let exist_block_with_code_sell =
            blockchain.exist_block_with_operation_code(OperationCode::Sell);

        if (exist_block_with_code_sell
            || exist_block_with_code_endorse
            || !my_peer_id.eq(&bill.payee.peer_id))
            && !(my_peer_id.eq(&bill.endorsee.peer_id))
        {
            return Err(Error::CallerIsNotPayeeOrEndorsee);
        }

        let identity = self.identity_store.get_full().await?;
        let data_for_new_block = self.get_data_for_new_block(
            &identity,
            "Sold to ",
            Some((&buyer, " sold by ")),
            &format!(" amount: {amount_numbers}"),
        )?;
        self.add_block_for_operation(
            bill_name,
            &mut blockchain,
            timestamp,
            OperationCode::Sell,
            identity,
            data_for_new_block,
        )
        .await?;
        Ok(blockchain)
    }

    async fn endorse_bitcredit_bill(
        &self,
        bill_name: &str,
        endorsee: IdentityPublicData,
        timestamp: i64,
    ) -> Result<Chain> {
        let my_peer_id = self.identity_store.get_peer_id().await?.to_string();
        let bill = read_bill_from_file(bill_name).await;

        let mut blockchain = Chain::read_chain_from_file(bill_name);

        let exist_block_with_code_endorse =
            blockchain.exist_block_with_operation_code(OperationCode::Endorse);

        let exist_block_with_code_mint =
            blockchain.exist_block_with_operation_code(OperationCode::Mint);

        let exist_block_with_code_sell =
            blockchain.exist_block_with_operation_code(OperationCode::Sell);

        if !((my_peer_id.eq(&bill.payee.peer_id)
            && !exist_block_with_code_endorse
            && !exist_block_with_code_sell
            && !exist_block_with_code_mint)
            || (my_peer_id.eq(&bill.endorsee.peer_id)))
        {
            return Err(Error::CallerIsNotPayeeOrEndorsee);
        }
        let identity = self.identity_store.get_full().await?;
        let data_for_new_block = self.get_data_for_new_block(
            &identity,
            "Endorsed to ",
            Some((&endorsee, " endorsed by ")),
            "",
        )?;
        self.add_block_for_operation(
            bill_name,
            &mut blockchain,
            timestamp,
            OperationCode::Endorse,
            identity,
            data_for_new_block,
        )
        .await?;

        Ok(blockchain)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        service::identity_service::Identity,
        tests::test::{TEST_PRIVATE_KEY, TEST_PUB_KEY},
    };
    use core::str;
    use futures::channel::mpsc;
    use libp2p::{identity::Keypair, PeerId};
    use mockall::predicate::{always, eq};
    use persistence::{bill::MockBillStoreApi, identity::MockIdentityStoreApi};
    use std::sync::Arc;
    use util::file::MockUploadFileHandler;

    fn get_service(mock_storage: MockBillStoreApi) -> BillService {
        let (sender, _) = mpsc::channel(0);
        BillService::new(
            Client::new(
                sender,
                Arc::new(MockBillStoreApi::new()),
                Arc::new(MockIdentityStoreApi::new()),
            ),
            Arc::new(mock_storage),
            Arc::new(MockIdentityStoreApi::new()),
        )
    }

    #[tokio::test]
    async fn issue_bill_baseline() {
        let expected_file_name = "invoice_00000000-0000-0000-0000-000000000000.pdf";
        let file_bytes = String::from("hello world").as_bytes().to_vec();

        let mut storage = MockBillStoreApi::new();
        storage
            .expect_write_bill_keys_to_file()
            .returning(|_, _, _| Ok(()));
        storage
            .expect_save_attached_file()
            .returning(move |_, _, _| Ok(()));

        let service = get_service(storage);

        let mut file = MockUploadFileHandler::new();
        file.expect_name()
            .returning(|| Some(String::from("invoice")));
        file.expect_extension()
            .returning(|| Some(String::from("pdf")));
        file.expect_get_contents()
            .returning(move || Ok(file_bytes.clone()));

        let mut identity = Identity::new_empty();
        identity.public_key_pem = TEST_PUB_KEY.to_owned();
        let drawer = IdentityWithAll {
            identity,
            peer_id: PeerId::random(),
            key_pair: Keypair::generate_ed25519(),
        };
        let drawee = IdentityPublicData::new_empty();
        let payee = IdentityPublicData::new_empty();

        let bill = service
            .issue_new_bill(
                String::from("UK"),
                String::from("Vienna"),
                100,
                String::from("London"),
                String::from("2030-01-01"),
                String::from("sa"),
                drawer,
                String::from("en"),
                drawee,
                payee,
                vec![&file],
            )
            .await
            .unwrap();

        assert_eq!(bill.files.first().unwrap().name, expected_file_name);
    }

    #[tokio::test]
    async fn save_encrypt_open_decrypt_compare_hashes() {
        let bill_name = "test_bill_name";
        let expected_file_name = "invoice_00000000-0000-0000-0000-000000000000.pdf";
        let file_bytes = String::from("hello world").as_bytes().to_vec();
        let expected_encrypted =
            util::rsa::encrypt_bytes_with_public_key(&file_bytes, TEST_PUB_KEY);

        let mut storage = MockBillStoreApi::new();
        storage
            .expect_save_attached_file()
            .with(always(), eq(bill_name), eq(expected_file_name))
            .times(1)
            .returning(|_, _, _| Ok(()));

        storage
            .expect_open_attached_file()
            .with(eq(bill_name), eq(expected_file_name))
            .times(1)
            .returning(move |_, _| Ok(expected_encrypted.clone()));
        let service = get_service(storage);

        let mut file = MockUploadFileHandler::new();
        file.expect_name()
            .returning(|| Some(String::from("invoice")));
        file.expect_extension()
            .returning(|| Some(String::from("pdf")));
        file.expect_get_contents()
            .returning(move || Ok(file_bytes.clone()));

        let bill_file = service
            .encrypt_and_save_uploaded_file(&file, bill_name, TEST_PUB_KEY)
            .await
            .unwrap();
        assert_eq!(
            bill_file.hash,
            String::from("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9")
        );
        assert_eq!(bill_file.name, String::from(expected_file_name));

        let decrypted = service
            .open_and_decrypt_attached_file(bill_name, expected_file_name, TEST_PRIVATE_KEY)
            .await
            .unwrap();
        assert_eq!(str::from_utf8(&decrypted).unwrap(), "hello world");
    }

    #[tokio::test]
    async fn save_encrypt_propagates_read_file_error() {
        let storage = MockBillStoreApi::new();
        let service = get_service(storage);

        let mut file = MockUploadFileHandler::new();
        file.expect_get_contents()
            .returning(|| Err(std::io::Error::new(std::io::ErrorKind::Other, "test error")));

        assert!(service
            .encrypt_and_save_uploaded_file(&file, "test", TEST_PUB_KEY)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn save_encrypt_propagates_write_file_error() {
        let mut storage = MockBillStoreApi::new();
        storage.expect_save_attached_file().returning(|_, _, _| {
            Err(persistence::Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "test error",
            )))
        });
        let service = get_service(storage);

        let mut file = MockUploadFileHandler::new();
        file.expect_name()
            .returning(|| Some(String::from("invoice")));
        file.expect_extension()
            .returning(|| Some(String::from("pdf")));
        file.expect_get_contents().returning(|| Ok(vec![]));

        assert!(service
            .encrypt_and_save_uploaded_file(&file, "test", TEST_PUB_KEY)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn save_encrypt_propagates_file_name_error() {
        let storage = MockBillStoreApi::new();
        let service = get_service(storage);

        let mut file = MockUploadFileHandler::new();
        file.expect_name().returning(|| None);
        file.expect_get_contents().returning(|| Ok(vec![]));

        assert!(service
            .encrypt_and_save_uploaded_file(&file, "test", TEST_PUB_KEY)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn open_decrypt_propagates_read_file_error() {
        let mut storage = MockBillStoreApi::new();
        storage.expect_open_attached_file().returning(|_, _| {
            Err(persistence::Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "test error",
            )))
        });
        let service = get_service(storage);

        assert!(service
            .open_and_decrypt_attached_file("test", "test", TEST_PRIVATE_KEY)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn get_bill_keys_calls_storage() {
        let mut storage = MockBillStoreApi::new();
        storage.expect_read_bill_keys_from_file().returning(|_| {
            Ok(BillKeys {
                private_key_pem: TEST_PRIVATE_KEY.to_owned(),
                public_key_pem: TEST_PUB_KEY.to_owned(),
            })
        });
        let service = get_service(storage);

        assert!(service.get_bill_keys("test").await.is_ok());
        assert_eq!(
            service.get_bill_keys("test").await.unwrap().private_key_pem,
            TEST_PRIVATE_KEY.to_owned()
        );
        assert_eq!(
            service.get_bill_keys("test").await.unwrap().public_key_pem,
            TEST_PUB_KEY.to_owned()
        );
    }

    #[tokio::test]
    async fn get_bill_keys_propagates_errors() {
        let mut storage = MockBillStoreApi::new();
        storage.expect_read_bill_keys_from_file().returning(|_| {
            Err(persistence::Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "test error",
            )))
        });
        let service = get_service(storage);

        assert!(service.get_bill_keys("test").await.is_err());
    }

    #[tokio::test]
    async fn validate_attached_file_checks_file_size() {
        let mut file = MockUploadFileHandler::new();
        file.expect_len()
            .returning(move || MAX_FILE_SIZE_BYTES as u64 * 2);

        let service = get_service(MockBillStoreApi::new());
        let res = service.validate_attached_file(&file).await;

        assert!(res.is_err());
    }

    #[tokio::test]
    async fn validate_attached_file_checks_file_name() {
        let mut file = MockUploadFileHandler::new();
        file.expect_len().returning(move || 100);
        file.expect_name().returning(move || None);

        let service = get_service(MockBillStoreApi::new());
        let res = service.validate_attached_file(&file).await;

        assert!(res.is_err());
    }

    #[tokio::test]
    async fn validate_attached_file_checks_file_name_empty() {
        let mut file = MockUploadFileHandler::new();
        file.expect_len().returning(move || 100);
        file.expect_name().returning(move || Some(String::from("")));

        let service = get_service(MockBillStoreApi::new());
        let res = service.validate_attached_file(&file).await;

        assert!(res.is_err());
    }

    #[tokio::test]
    async fn validate_attached_file_checks_file_name_length() {
        let mut file = MockUploadFileHandler::new();
        file.expect_len().returning(move || 100);
        file.expect_name()
            .returning(move || Some(String::from("veryververyververyververyververyververyververyververyververyververyververyververyververyververyververyververyververyververyveryyyyyyyyyyyyyyyyyyveryverylongname")));

        let service = get_service(MockBillStoreApi::new());
        let res = service.validate_attached_file(&file).await;

        assert!(res.is_err());
    }

    #[tokio::test]
    async fn validate_attached_file_checks_file_type_error() {
        let mut file = MockUploadFileHandler::new();
        file.expect_len().returning(move || 100);
        file.expect_name()
            .returning(move || Some(String::from("goodname")));
        file.expect_detect_content_type()
            .returning(move || Err(std::io::Error::new(std::io::ErrorKind::Other, "test error")));

        let service = get_service(MockBillStoreApi::new());
        let res = service.validate_attached_file(&file).await;

        assert!(res.is_err());
    }

    #[tokio::test]
    async fn validate_attached_file_checks_file_type_invalid() {
        let mut file = MockUploadFileHandler::new();
        file.expect_len().returning(move || 100);
        file.expect_name()
            .returning(move || Some(String::from("goodname")));
        file.expect_detect_content_type()
            .returning(move || Ok(None));

        let service = get_service(MockBillStoreApi::new());
        let res = service.validate_attached_file(&file).await;

        assert!(res.is_err());
    }

    #[tokio::test]
    async fn validate_attached_file_checks_file_type_not_in_list() {
        let mut file = MockUploadFileHandler::new();
        file.expect_len().returning(move || 100);
        file.expect_name()
            .returning(move || Some(String::from("goodname")));
        file.expect_detect_content_type()
            .returning(move || Ok(Some(String::from("invalidfile"))));

        let service = get_service(MockBillStoreApi::new());
        let res = service.validate_attached_file(&file).await;

        assert!(res.is_err());
    }

    #[tokio::test]
    async fn validate_attached_file_checks_valid() {
        let mut file = MockUploadFileHandler::new();
        file.expect_len().returning(move || 100);
        file.expect_name()
            .returning(move || Some(String::from("goodname")));
        file.expect_detect_content_type()
            .returning(move || Ok(Some(String::from("application/pdf"))));

        let service = get_service(MockBillStoreApi::new());
        let res = service.validate_attached_file(&file).await;

        assert!(res.is_ok());
    }
    // TODO: tests for add_block for operation
    // TODO: tests for get_data_for_new_block
}
