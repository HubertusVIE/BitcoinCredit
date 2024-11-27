use super::behaviour::{
    file_request_for_bill, file_request_for_bill_attachment, file_request_for_bill_keys,
    parse_inbound_file_request, BillAttachmentFileRequest, BillFileRequest, BillKeysFileRequest,
    Command, Event, FileResponse, ParsedInboundFileRequest,
};
use super::Result;
use crate::blockchain::{Chain, GossipsubEvent, GossipsubEventId};
use crate::constants::{BILLS_PREFIX, INFO_PREFIX};
use crate::persistence::bill::BillStoreApi;
use crate::persistence::company::CompanyStoreApi;
use crate::persistence::identity::IdentityStoreApi;
use crate::service::company_service::{Company, CompanyKeys, CompanyPublicData};
use crate::service::contact_service::IdentityPublicData;
use crate::util;
use crate::util::rsa::{decrypt_bytes_with_private_key, encrypt_bytes_with_public_key};
use future::try_join_all;
use futures::channel::mpsc::Receiver;
use futures::channel::{mpsc, oneshot};
use futures::prelude::*;
use libp2p::kad::record::Record;
use libp2p::request_response::ResponseChannel;
use libp2p::PeerId;
use log::{error, info};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::broadcast;

/// The DHT client
#[derive(Clone)]
pub struct Client {
    /// The sender to send events to the event loop
    pub(super) sender: mpsc::Sender<Command>,
    bill_store: Arc<dyn BillStoreApi>,
    company_store: Arc<dyn CompanyStoreApi>,
    identity_store: Arc<dyn IdentityStoreApi>,
}

impl Client {
    pub fn new(
        sender: mpsc::Sender<Command>,
        bill_store: Arc<dyn BillStoreApi>,
        company_store: Arc<dyn CompanyStoreApi>,
        identity_store: Arc<dyn IdentityStoreApi>,
    ) -> Self {
        Self {
            sender,
            bill_store,
            company_store,
            identity_store,
        }
    }

    /// Runs the dht client, listening for incoming events from the event loop
    pub async fn run(
        mut self,
        mut network_events: Receiver<Event>,
        mut shutdown_dht_client_receiver: broadcast::Receiver<bool>,
    ) {
        loop {
            tokio::select! {
                event = network_events.next() => self.handle_event(event.expect("Swarm stream to be infinite.")).await,
                _ = shutdown_dht_client_receiver.recv() => {
                    info!("Shutting down dht client...");
                    break;
                }
            }
        }
    }

    // --------------------------------------------------------------
    // Company-related logic ---------------------------------------
    // --------------------------------------------------------------

    pub async fn put_companies_public_data_in_dht(&mut self) -> Result<()> {
        let companies = self.company_store.get_all().await?;
        log::info!("Putting local company public data in the DHT");

        if !companies.is_empty() {
            let tasks = companies.into_iter().map(|(id, (company, company_keys))| {
                let mut self_clone = self.clone();
                async move {
                    self_clone
                        .put_company_public_data_in_dht((id, (company, company_keys)))
                        .await
                }
            });

            try_join_all(tasks).await?;
        }

        Ok(())
    }

    async fn put_company_public_data_in_dht(
        &mut self,
        company_data: (String, (Company, CompanyKeys)),
    ) -> Result<()> {
        let id = company_data.0;
        let local_public_data =
            CompanyPublicData::from(id.clone(), company_data.1 .0, company_data.1 .1);
        let local_json = serde_json::to_string(&local_public_data)?;

        let dht_record = self.get_record(id.clone()).await?.value;
        if dht_record.is_empty() {
            self.put_record(id.clone(), local_json).await?;
        } else {
            let dht_public_data: CompanyPublicData = serde_json::from_slice(&dht_record)?;
            if !dht_public_data.eq(&local_public_data) {
                self.put_record(id.clone(), local_json).await?;
            }
        }
        Ok(())
    }

    // --------------------------------------------------------------
    // Identity-related logic ---------------------------------------
    // --------------------------------------------------------------

    /// Adds the local identity's public data into the DHT
    pub async fn put_identity_public_data_in_dht(&mut self) -> Result<()> {
        if self.identity_store.exists().await {
            let identity = self.identity_store.get_full().await?;
            let identity_data = IdentityPublicData::new(
                identity.identity.clone(),
                identity.peer_id.to_string().clone(),
            );

            let key = format!("{}{}", INFO_PREFIX, &identity_data.peer_id);
            let current_info = self.get_record(key.clone()).await?.value;
            let mut current_info_string = String::new();
            if !current_info.is_empty() {
                current_info_string = std::str::from_utf8(&current_info)?.to_string();
            }
            let value = serde_json::to_string(&identity_data)?;
            if !current_info_string.eq(&value) {
                self.put_record(key, value).await?;
            }
        }
        Ok(())
    }

    /// Queries the DHT for the public identity data for the given peer id
    pub async fn get_identity_public_data_from_dht(
        &mut self,
        peer_id: String,
    ) -> Result<IdentityPublicData> {
        let key = format!("{}{}", INFO_PREFIX, &peer_id);
        let current_info = self.get_record(key.clone()).await?.value;
        let mut identity_public_data: IdentityPublicData = IdentityPublicData::new_empty();
        if !current_info.is_empty() {
            let current_info_string = std::str::from_utf8(&current_info)?.to_string();
            identity_public_data = serde_json::from_str(&current_info_string)?;
        }

        Ok(identity_public_data)
    }

    // ---------------------------------------------------------
    // Bill-related logic --------------------------------------
    // ---------------------------------------------------------

    /// Checks the bill record for the local node, adding missing bills and starts providing them, if necessary
    pub async fn update_bills_table(&mut self, node_id: String) -> Result<()> {
        let node_request = BILLS_PREFIX.to_string() + &node_id;
        let list_bills_for_node = self.get_record(node_request.clone()).await?;
        let value = list_bills_for_node.value;

        if !value.is_empty() {
            let record_in_dht = std::str::from_utf8(&value)?.to_string();
            let mut new_record: String = record_in_dht.clone();

            let bill_names = self.bill_store.get_bill_names().await?;
            for bill_name in bill_names {
                if !record_in_dht.contains(&bill_name) {
                    new_record += (",".to_string() + &bill_name.clone()).as_str();
                    self.start_providing(bill_name.clone()).await?;
                }
            }
            if !record_in_dht.eq(&new_record) {
                self.put_record(node_request.clone(), new_record).await?;
            }
        } else {
            let mut new_record = String::new();
            let bill_names = self.bill_store.get_bill_names().await?;
            for bill_name in bill_names {
                if new_record.is_empty() {
                    new_record = bill_name.clone();
                    self.start_providing(bill_name.clone()).await?;
                } else {
                    new_record += (",".to_string() + &bill_name.clone()).as_str();
                    self.start_providing(bill_name.clone()).await?;
                }
            }
            if !new_record.is_empty() {
                self.put_record(node_request.clone(), new_record).await?;
            }
        }
        Ok(())
    }

    /// Starts providing all locally available bills
    pub async fn start_providing_bills(&mut self) -> Result<()> {
        let bills = self.bill_store.get_bill_names().await?;
        for bill in bills {
            self.start_providing(bill).await?;
        }
        Ok(())
    }

    /// Adds the given bill for the given node id
    pub async fn add_bill_to_dht_for_node(&mut self, bill_name: &str, node_id: &str) -> Result<()> {
        let node_request = BILLS_PREFIX.to_string() + node_id;
        let mut record_for_saving_in_dht;
        let list_bills_for_node = self.get_record(node_request.clone()).await?;
        let value = list_bills_for_node.value;
        if !value.is_empty() {
            record_for_saving_in_dht = std::str::from_utf8(&value)?.to_string();
            if !record_for_saving_in_dht.contains(bill_name) {
                record_for_saving_in_dht = record_for_saving_in_dht.to_string() + "," + bill_name;
            }
        } else {
            record_for_saving_in_dht = bill_name.to_owned();
        }

        if !std::str::from_utf8(&value)?
            .to_string()
            .eq(&record_for_saving_in_dht)
        {
            self.put_record(node_request.clone(), record_for_saving_in_dht.to_string())
                .await?;
        }
        Ok(())
    }

    /// Checks the DHT for new bills, fetching the bill data, keys and files for them
    pub async fn check_new_bills(&mut self, node_id: String) -> Result<()> {
        let node_request = BILLS_PREFIX.to_string() + &node_id;
        let list_bills_for_node = self.get_record(node_request.clone()).await?;
        let value = list_bills_for_node.value;

        if !value.is_empty() {
            let record_for_saving_in_dht = std::str::from_utf8(&value)?.to_string();
            let bills = record_for_saving_in_dht.split(',');
            for bill_id in bills {
                if !self.bill_store.bill_exists(bill_id).await {
                    let bill_bytes = self.get_bill(bill_id).await?;
                    self.bill_store
                        .write_bill_to_file(bill_id, &bill_bytes)
                        .await?;

                    let key_bytes = self.get_key(bill_id).await?;
                    let pr_key = self
                        .identity_store
                        .get_full()
                        .await?
                        .identity
                        .private_key_pem;

                    let key_bytes_decrypted = decrypt_bytes_with_private_key(&key_bytes, pr_key);
                    self.bill_store
                        .write_bill_keys_to_file_as_bytes(bill_id, &key_bytes_decrypted)
                        .await?;

                    if let Ok(chain) = self.bill_store.read_bill_chain_from_file(bill_id).await {
                        let bill = chain.get_first_version_bill();
                        for file in bill.files {
                            if let Err(e) = self.get_bill_attachment(bill_id, &file.name).await {
                                error!("Could not get bill attachment with file name {} for bill {bill_id}: {e}", file.name);
                            }
                        }
                    }

                    self.sender
                        .send(Command::SubscribeToTopic {
                            topic: bill_id.to_string().clone(),
                        })
                        .await?;
                }
            }
        }
        Ok(())
    }

    /// Requests the data file for the given bill
    pub async fn get_bill(&mut self, name: &str) -> Result<Vec<u8>> {
        let local_peer_id = self.identity_store.get_peer_id().await?;
        let mut providers = self.get_providers(name.to_owned()).await?;
        providers.remove(&local_peer_id);
        if providers.is_empty() {
            return Err(super::Error::NoProviders(format!(
                "Get Bill: No providers found for {name}",
            )));
        }
        let requests = providers.into_iter().map(|peer| {
            let mut network_client = self.clone();

            let file_request = file_request_for_bill(&local_peer_id.to_string(), name);
            async move { network_client.request_file(peer, file_request).await }.boxed()
        });

        match futures::future::select_ok(requests).await {
            Err(e) => Err(super::Error::NoProviders(format!(
                "Get Bill: None of the providers returned the file for {name}: {e}",
            ))),
            Ok(file_content) => Ok(file_content.0),
        }
    }

    /// Requests the given file for the given bill name, decrypting it, checking it's hash,
    /// encrypting it and saving it once it arrives
    pub async fn get_bill_attachment(&mut self, bill_name: &str, file_name: &str) -> Result<()> {
        // check if there is such a bill and if it contains this file
        let bill = self
            .bill_store
            .read_bill_chain_from_file(bill_name)
            .await?
            .get_first_version_bill();
        let local_hash = match bill.files.iter().find(|file| file.name.eq(file_name)) {
            None => {
                return Err(super::Error::FileNotFoundInBill(format!("Get Bill Attachment: No file found in bill {bill_name} with file name {file_name}")));
            }
            Some(file) => &file.hash,
        };

        let local_peer_id = self.identity_store.get_peer_id().await?;
        let mut providers = self.get_providers(bill_name.to_owned()).await?;
        providers.remove(&local_peer_id);
        if providers.is_empty() {
            return Err(super::Error::NoProviders(format!(
                "Get Bill Attachment: No providers found for {bill_name}",
            )));
        }

        let requests = providers.into_iter().map(|peer_id| {
            let mut network_client = self.clone();
            let file_request =
                file_request_for_bill_attachment(&local_peer_id.to_string(), bill_name, file_name);
            async move { network_client.request_file(peer_id, file_request).await }.boxed()
        });

        match futures::future::select_ok(requests).await {
            Err(e) => Err(super::Error::NoFileFromProviders(format!(
                "Get Bill Attachment: None of the providers returned the file for {bill_name}: {e}"
            ))),
            Ok(file_content) => {
                let bytes = file_content.0;
                let keys = self.bill_store.read_bill_keys_from_file(bill_name).await?;
                let pr_key = self
                    .identity_store
                    .get_full()
                    .await?
                    .identity
                    .private_key_pem;
                // decrypt file using identity private key and check hash
                let decrypted_with_identity_key =
                    util::rsa::decrypt_bytes_with_private_key(&bytes, pr_key);
                let decrypted_with_bill_key = util::rsa::decrypt_bytes_with_private_key(
                    &decrypted_with_identity_key,
                    keys.private_key_pem,
                );
                let remote_hash = util::sha256_hash(&decrypted_with_bill_key);
                if local_hash != remote_hash.as_str() {
                    return Err(super::Error::FileHashesDidNotMatch(format!("Get Bill Attachment: Hashes didn't match for bill {bill_name} and file name {file_name}, remote: {remote_hash}, local: {local_hash}")));
                }
                // encrypt with bill public key and save file locally
                let encrypted = util::rsa::encrypt_bytes_with_public_key(
                    &decrypted_with_bill_key,
                    &keys.public_key_pem,
                );
                self.bill_store
                    .save_attached_file(&encrypted, bill_name, file_name)
                    .await?;
                Ok(())
            }
        }
    }

    /// Requests the keys file for the given bill
    pub async fn get_key(&mut self, name: &str) -> Result<Vec<u8>> {
        let local_peer_id = self.identity_store.get_peer_id().await?;
        let mut providers = self.get_providers(name.to_owned()).await?;
        providers.remove(&local_peer_id);
        if providers.is_empty() {
            return Err(super::Error::NoProviders(format!(
                "Get Bill Keys: No providers found for {name}",
            )));
        }
        let requests = providers.into_iter().map(|peer| {
            let mut network_client = self.clone();

            let file_request = file_request_for_bill_keys(&local_peer_id.to_string(), name);
            async move { network_client.request_file(peer, file_request).await }.boxed()
        });

        match futures::future::select_ok(requests).await {
            Err(e) => Err(super::Error::NoFileFromProviders(format!(
                "Get Bill Keys: None of the providers returned the file for {name}: {e}"
            ))),
            Ok(file_content) => Ok(file_content.0),
        }
    }

    /// Puts all participants of every local bill to the record of the respective bill in the DHT
    pub async fn put_bills_for_parties(&mut self) -> Result<()> {
        let bills = self.bill_store.get_bills().await?;

        for bill in bills {
            let chain = Chain::read_chain_from_file(&bill.name);
            let nodes = chain.get_all_nodes_from_bill();
            for node in nodes {
                self.add_bill_to_dht_for_node(&bill.name, &node).await?;
            }
        }
        Ok(())
    }

    /// Subscribes to all locally available bills
    pub async fn subscribe_to_all_bills_topics(&mut self) -> Result<()> {
        let bills = self.bill_store.get_bills().await?;

        for bill in bills {
            self.subscribe_to_topic(bill.name).await?;
        }
        Ok(())
    }

    /// Asks on the topic to receive the current chain of all local bills
    pub async fn receive_updates_for_all_bills_topics(&mut self) -> Result<()> {
        let bills = self.bill_store.get_bills().await?;

        for bill in bills {
            let event = GossipsubEvent::new(GossipsubEventId::CommandGetChain, vec![0; 24]);
            let message = event.to_byte_array();

            self.add_message_to_topic(message, bill.name).await?;
        }
        Ok(())
    }

    // -------------------------------------------------------------
    // Utility Functions for the DHT -------------------------------
    // -------------------------------------------------------------

    /// Sends the given message to the given topic via the event loop
    pub async fn add_message_to_topic(&mut self, msg: Vec<u8>, topic: String) -> Result<()> {
        self.send_message(msg, topic).await?;
        Ok(())
    }

    /// Subscribe to the given topic via the event loop
    pub async fn subscribe_to_topic(&mut self, topic: String) -> Result<()> {
        self.sender
            .send(Command::SubscribeToTopic { topic })
            .await?;
        Ok(())
    }

    /// Send the given message to the given topic via the event loop
    pub async fn send_message(&mut self, msg: Vec<u8>, topic: String) -> Result<()> {
        self.sender
            .send(Command::SendMessage { msg, topic })
            .await?;
        Ok(())
    }

    /// Puts the given value into the DHT at the given key via the event loop
    pub async fn put_record(&mut self, key: String, value: String) -> Result<()> {
        self.sender.send(Command::PutRecord { key, value }).await?;
        Ok(())
    }

    /// Gets the record for the given key via the event loop
    pub async fn get_record(&mut self, key: String) -> Result<Record> {
        let (sender, receiver) = oneshot::channel();
        self.sender.send(Command::GetRecord { key, sender }).await?;
        let record = receiver.await?;
        Ok(record)
    }

    /// Adds the current node to the list of providers for the given entry via the event loop
    pub async fn start_providing(&mut self, entry: String) -> Result<()> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::StartProviding { entry, sender })
            .await?;
        receiver.await?;
        Ok(())
    }

    /// Gets the providers for the given file via the event loop
    pub async fn get_providers(&mut self, entry: String) -> Result<HashSet<PeerId>> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::GetProviders { entry, sender })
            .await
            .expect("Command receiver not to be dropped.");
        let providers = receiver.await?;
        Ok(providers)
    }

    /// Request the given file via the event loop
    async fn request_file(&mut self, peer: PeerId, file_name: String) -> Result<Vec<u8>> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::RequestFile {
                file_name,
                peer,
                sender,
            })
            .await?;
        let res = receiver.await?;
        res.map_err(|e| super::Error::RequestFile(e.to_string()))
    }

    /// Respond with the given file via the event loop
    async fn respond_file(
        &mut self,
        file: Vec<u8>,
        channel: ResponseChannel<FileResponse>,
    ) -> Result<()> {
        self.sender
            .send(Command::RespondFile { file, channel })
            .await?;
        Ok(())
    }

    // -------------------------------------------------------------
    // Request handling code ---------------------------------------
    // -------------------------------------------------------------

    /// Handles incoming requests
    async fn handle_event(&mut self, event: Event) {
        let Event::InboundRequest { request, channel } = event;
        match parse_inbound_file_request(&request) {
            Err(e) => {
                error!("Could not handle inbound request {request}: {e}")
            }
            Ok(parsed) => {
                match parsed {
                    // We can send the bill to anyone requesting it, since the content is encrypted
                    // and is useless without the keys
                    ParsedInboundFileRequest::Bill(BillFileRequest { bill_name }) => {
                        match self.bill_store.get_bill_as_bytes(&bill_name).await {
                            Err(e) => {
                                error!("Could not handle inbound request {request}: {e}")
                            }
                            Ok(file) => {
                                if let Err(e) = self.respond_file(file, channel).await {
                                    error!("Could not handle inbound request {request} - error responding with file: {e}")
                                }
                            }
                        }
                    }
                    // We check if the requester is part of the bill and if so, we get their
                    // identity from DHT and encrypt the file with their public key
                    ParsedInboundFileRequest::BillKeys(BillKeysFileRequest {
                        node_id,
                        key_name,
                    }) => {
                        let chain = Chain::read_chain_from_file(&key_name);
                        if chain.bill_contains_node(&node_id) {
                            match self.get_identity_public_data_from_dht(node_id).await {
                                Err(e) => {
                                    error!("Could not handle inbound request {request} - could not get identity public data for: {e}", )
                                }
                                Ok(data) => {
                                    let public_key = data.rsa_public_key_pem;
                                    match self.bill_store.get_bill_keys_as_bytes(&key_name).await {
                                        Err(e) => {
                                            error!(
                                                "Could not handle inbound request {request}: {e}"
                                            )
                                        }
                                        Ok(file) => {
                                            let file_encrypted =
                                                encrypt_bytes_with_public_key(&file, &public_key);

                                            if let Err(e) =
                                                self.respond_file(file_encrypted, channel).await
                                            {
                                                error!("Could not handle inbound request {request} - error responding with file: {e}")
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // We only send attachments (encrypted with the bill public key) to participants of the bill, encrypted with their public key
                    ParsedInboundFileRequest::BillAttachment(BillAttachmentFileRequest {
                        node_id,
                        bill_name,
                        file_name,
                    }) => {
                        let chain = Chain::read_chain_from_file(&bill_name);
                        if chain.bill_contains_node(&node_id) {
                            match self.get_identity_public_data_from_dht(node_id).await {
                                Err(e) => {
                                    error!("Could not handle inbound request {request} - could not get identity public data for: {e}");
                                }
                                Ok(data) => {
                                    let public_key = data.rsa_public_key_pem;

                                    match self
                                        .bill_store
                                        .open_attached_file(&bill_name, &file_name)
                                        .await
                                    {
                                        Err(e) => {
                                            error!(
                                                "Could not handle inbound request {request}: {e}"
                                            )
                                        }
                                        Ok(file) => {
                                            let file_encrypted =
                                                encrypt_bytes_with_public_key(&file, &public_key);

                                            if let Err(e) =
                                                self.respond_file(file_encrypted, channel).await
                                            {
                                                error!("Could not handle inbound request {request} - error responding with file: {e}")
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
