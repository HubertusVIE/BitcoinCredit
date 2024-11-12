use super::behaviour::{
    file_request_for_bill, file_request_for_bill_attachment, file_request_for_bill_keys,
    parse_inbound_file_request, BillAttachmentFileRequest, BillFileRequest, BillKeysFileRequest,
    Command, Event, FileResponse, ParsedInboundFileRequest,
};
use crate::bill::{get_path_for_bill, get_path_for_bill_keys};
use crate::blockchain::{Chain, GossipsubEvent, GossipsubEventId};
use crate::constants::{BILLS_FOLDER_PATH, BILLS_PREFIX, IDENTITY_FILE_PATH};
use crate::persistence::bill::BillStoreApi;
use crate::service::contact_service::IdentityPublicData;
use crate::util;
use crate::{
    bill::{
        get_bills,
        identity::{get_whole_identity, read_peer_id_from_file, IdentityWithAll},
    },
    util::{
        file::is_not_hidden_or_directory,
        rsa::{decrypt_bytes_with_private_key, encrypt_bytes_with_public_key},
    },
};
use anyhow::{anyhow, Result};
use futures::channel::mpsc::Receiver;
use futures::channel::{mpsc, oneshot};
use futures::prelude::*;
use libp2p::kad::record::Record;
use libp2p::request_response::ResponseChannel;
use libp2p::PeerId;
use log::{error, info};
use std::collections::HashSet;
use std::fs;
use std::io::BufRead;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct Client {
    pub(super) sender: mpsc::Sender<Command>,
    bill_store: Arc<dyn BillStoreApi>,
}

impl Client {
    pub fn new(sender: mpsc::Sender<Command>, bill_store: Arc<dyn BillStoreApi>) -> Self {
        Self { sender, bill_store }
    }

    pub async fn run(
        mut self,
        mut network_events: Receiver<Event>,
        mut shutdown_dht_client_receiver: broadcast::Receiver<bool>,
    ) {
        // We need to use blocking stdin, because tokio's async stdin isn't meant for interactive
        // use-cases and will block forever on finishing the program
        let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::channel(100);
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            for line in stdin.lock().lines() {
                match line {
                    Ok(line) => {
                        let line = line.trim().to_string();
                        if !line.is_empty() {
                            if let Err(e) = stdin_tx.blocking_send(line) {
                                error!("Error handling stdin: {e}");
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error reading line from stdin: {e}");
                    }
                }
            }
        });

        loop {
            tokio::select! {
                line = stdin_rx.recv() => {
                    if let Some(next_line) = line {
                        self.handle_input_line(next_line).await
                    }
                },
                event = network_events.next() => self.handle_event(event.expect("Swarm stream to be infinite.")).await,
                _ = shutdown_dht_client_receiver.recv() => {
                    info!("Shutting down dht client...");
                    break;
                }
            }
        }
    }

    pub async fn check_new_bills(&mut self, node_id: String) {
        let node_request = BILLS_PREFIX.to_string() + &node_id;
        let list_bills_for_node = self.get_record(node_request.clone()).await;
        let value = list_bills_for_node.value;

        if !value.is_empty() {
            let record_for_saving_in_dht = std::str::from_utf8(&value)
                .expect("Cant get value.")
                .to_string();
            let bills = record_for_saving_in_dht.split(',');
            for bill_id in bills {
                let path = get_path_for_bill(bill_id);
                let path_for_keys = get_path_for_bill_keys(bill_id);
                if !path.exists() {
                    let bill_bytes = self.get_bill(bill_id.to_string().clone()).await;
                    if !bill_bytes.is_empty() {
                        fs::write(path, bill_bytes.clone()).expect("Can't write file.");
                    }

                    let key_bytes = self.get_key(bill_id.to_string().clone()).await;
                    if !key_bytes.is_empty() {
                        let pr_key = get_whole_identity().identity.private_key_pem;

                        let key_bytes_decrypted =
                            decrypt_bytes_with_private_key(&key_bytes, pr_key);

                        fs::write(path_for_keys, key_bytes_decrypted).expect("Can't write file.");
                    }

                    if !bill_bytes.is_empty() {
                        if let Ok(chain) = self.bill_store.read_bill_chain_from_file(bill_id).await
                        {
                            let bill = chain.get_first_version_bill();
                            for file in bill.files {
                                if let Err(e) = self
                                    .get_bill_attachment(bill_id.to_owned(), file.name.clone())
                                    .await
                                {
                                    error!("Could not get bill attachment with file name {} for bill {bill_id}: {e}", file.name);
                                }
                            }
                        }
                    }

                    if !bill_bytes.is_empty() {
                        self.sender
                            .send(Command::SubscribeToTopic {
                                topic: bill_id.to_string().clone(),
                            })
                            .await
                            .expect("Command receiver not to be dropped.");
                    }
                }
            }
        }
    }

    //TODO: change
    //
    // pub async fn upgrade_table_for_other_node(&mut self, node_id: String, bill: String) {
    //     let node_request = BILLS_PREFIX.to_string() + &node_id;
    //     let list_bills_for_node = self.get_record(node_request.clone()).await;
    //     let value = list_bills_for_node.value;
    //
    //     if !value.is_empty() {
    //         let record_in_dht = std::str::from_utf8(&value)
    //             .expect("Cant get value.")
    //             .to_string();
    //         let mut new_record: String = record_in_dht.clone();
    //
    //         if !record_in_dht.contains(&bill) {
    //             new_record += (",".to_string() + &bill).as_str();
    //         }
    //
    //         if !record_in_dht.eq(&new_record) {
    //             self.put_record(node_request.clone(), new_record).await;
    //         }
    //     } else {
    //         let mut new_record: String = bill.clone();
    //
    //         if !new_record.is_empty() {
    //             self.put_record(node_request.clone(), new_record).await;
    //         }
    //     }
    // }

    pub async fn upgrade_table(&mut self, node_id: String) {
        let node_request = BILLS_PREFIX.to_string() + &node_id;
        let list_bills_for_node = self.get_record(node_request.clone()).await;
        let value = list_bills_for_node.value;

        if !value.is_empty() {
            let record_in_dht = std::str::from_utf8(&value)
                .expect("Cant get value.")
                .to_string();
            let mut new_record: String = record_in_dht.clone();

            for file in fs::read_dir(BILLS_FOLDER_PATH).unwrap() {
                let dir = file.unwrap();
                if is_not_hidden_or_directory(&dir) {
                    let bill_name = dir
                        .path()
                        .file_stem()
                        .expect("File name error")
                        .to_str()
                        .expect("File name error")
                        .to_owned();

                    if !record_in_dht.contains(&bill_name) {
                        new_record += (",".to_string() + &bill_name.clone()).as_str();
                        self.put(&bill_name).await;
                    }
                }
            }
            if !record_in_dht.eq(&new_record) {
                self.put_record(node_request.clone(), new_record).await;
            }
        } else {
            let mut new_record = String::new();
            for file in fs::read_dir(BILLS_FOLDER_PATH).unwrap() {
                let dir = file.unwrap();
                if is_not_hidden_or_directory(&dir) {
                    let bill_name = dir
                        .path()
                        .file_stem()
                        .expect("File name error")
                        .to_str()
                        .expect("File name error")
                        .to_owned();

                    if new_record.is_empty() {
                        new_record = bill_name.clone();
                        self.put(&bill_name).await;
                    } else {
                        new_record += (",".to_string() + &bill_name.clone()).as_str();
                        self.put(&bill_name).await;
                    }
                }
            }
            if !new_record.is_empty() {
                self.put_record(node_request.clone(), new_record).await;
            }
        }
    }

    pub async fn start_provide(&mut self) {
        for file in fs::read_dir(BILLS_FOLDER_PATH).unwrap() {
            let dir = file.unwrap();
            if is_not_hidden_or_directory(&dir) {
                let bill_name = dir
                    .path()
                    .file_stem()
                    .expect("File name error")
                    .to_str()
                    .expect("File name error")
                    .to_owned();
                self.put(&bill_name).await;
            }
        }
    }

    pub async fn put_identity_public_data_in_dht(&mut self) {
        if Path::new(IDENTITY_FILE_PATH).exists() {
            let identity: IdentityWithAll = get_whole_identity();
            let identity_data = IdentityPublicData::new(
                identity.identity.clone(),
                identity.peer_id.to_string().clone(),
            );

            let key = "INFO".to_string() + &identity_data.peer_id;
            let current_info = self.get_record(key.clone()).await.value;
            let mut current_info_string = String::new();
            if !current_info.is_empty() {
                current_info_string = std::str::from_utf8(&current_info)
                    .expect("Cant get value.")
                    .to_string();
            }
            let value = serde_json::to_string(&identity_data).unwrap();
            if !current_info_string.eq(&value) {
                self.put_record(key, value).await;
            }
        }
    }

    pub async fn get_identity_public_data_from_dht(
        &mut self,
        peer_id: String,
    ) -> IdentityPublicData {
        let key = "INFO".to_string() + &peer_id;
        let current_info = self.get_record(key.clone()).await.value;
        let mut identity_public_data: IdentityPublicData = IdentityPublicData::new_empty();
        if !current_info.is_empty() {
            let current_info_string = std::str::from_utf8(&current_info)
                .expect("Cant get value.")
                .to_string();
            identity_public_data = serde_json::from_str(&current_info_string).unwrap();
        }

        identity_public_data
    }

    pub async fn add_bill_to_dht_for_node(&mut self, bill_name: &str, node_id: &str) {
        let node_request = BILLS_PREFIX.to_string() + node_id;
        let mut record_for_saving_in_dht;
        let list_bills_for_node = self.get_record(node_request.clone()).await;
        let value = list_bills_for_node.value;
        if !value.is_empty() {
            record_for_saving_in_dht = std::str::from_utf8(&value)
                .expect("Cant get value.")
                .to_string();
            if !record_for_saving_in_dht.contains(bill_name) {
                record_for_saving_in_dht = record_for_saving_in_dht.to_string() + "," + bill_name;
            }
        } else {
            record_for_saving_in_dht = bill_name.to_owned();
        }

        if !std::str::from_utf8(&value)
            .expect("Cant get value.")
            .to_string()
            .eq(&record_for_saving_in_dht)
        {
            self.put_record(node_request.clone(), record_for_saving_in_dht.to_string())
                .await;
        }
    }

    pub async fn add_message_to_topic(&mut self, msg: Vec<u8>, topic: String) {
        self.send_message(msg, topic).await;
    }

    pub async fn put(&mut self, name: &str) {
        self.start_providing(name.to_owned()).await;
    }

    pub async fn get_bill(&mut self, name: String) -> Vec<u8> {
        let providers = self.get_providers(name.clone()).await;
        if providers.is_empty() {
            error!("No providers was found.");
            Vec::new()
        } else {
            //TODO: If it's me - don't continue.
            let requests = providers.into_iter().map(|peer| {
                let mut network_client = self.clone();
                let local_peer_id = read_peer_id_from_file().to_string();

                let file_request = file_request_for_bill(&local_peer_id, &name);
                async move { network_client.request_file(peer, file_request).await }.boxed()
            });

            let file_content = futures::future::select_ok(requests);

            let file_content_await = file_content.await;

            if file_content_await.is_err() {
                error!("None of the providers returned file.");
                Vec::new()
            } else {
                file_content_await
                    .map_err(|_| "None of the providers returned file.")
                    .expect("Can not get file content.")
                    .0
            }
        }
    }

    /// Requests the given file for the given bill name, decrypting it, checking it's hash,
    /// encrypting it and saving it once it arrives
    pub async fn get_bill_attachment(
        &mut self,
        bill_name: String,
        file_name: String,
    ) -> Result<()> {
        // check if there is such a bill and if it contains this file
        let bill = self
            .bill_store
            .read_bill_chain_from_file(&bill_name)
            .await?
            .get_first_version_bill();
        let local_hash = match bill.files.iter().find(|file| file.name.eq(&file_name)) {
            None => {
                return Err(anyhow!("Get Bill Attachment: No file found in bill {bill_name} with file name {file_name}"));
            }
            Some(file) => &file.hash,
        };

        let local_peer_id = read_peer_id_from_file();
        let mut providers = self.get_providers(bill_name.to_owned()).await;
        providers.remove(&local_peer_id);
        if providers.is_empty() {
            return Err(anyhow!("Get Bill Attachment: No providers found"));
        }

        let requests = providers.into_iter().map(|peer_id| {
            let mut network_client = self.clone();
            let file_request = file_request_for_bill_attachment(
                &local_peer_id.to_string(),
                &bill_name,
                &file_name,
            );
            async move { network_client.request_file(peer_id, file_request).await }.boxed()
        });

        match futures::future::select_ok(requests).await {
            Err(e) => Err(anyhow!(
                "Get Bill Attachment: None of the providers returned the file: {e}"
            )),
            Ok(file_content) => {
                let bytes = file_content.0;
                let keys = self.bill_store.read_bill_keys_from_file(&bill_name).await?;
                let pr_key = get_whole_identity().identity.private_key_pem;
                // decrypt file using identity private key and check hash
                let decrypted_with_identity_key =
                    util::rsa::decrypt_bytes_with_private_key(&bytes, pr_key);
                let decrypted_with_bill_key = util::rsa::decrypt_bytes_with_private_key(
                    &decrypted_with_identity_key,
                    keys.private_key_pem,
                );
                let remote_hash = util::sha256_hash(&decrypted_with_bill_key);
                if local_hash != remote_hash.as_str() {
                    return Err(anyhow!("Get Bill Attachment: Hashes didn't match for bill {bill_name} and file name {file_name}, remote: {remote_hash}, local: {local_hash}"));
                }
                // encrypt with bill public key and save file locally
                let encrypted = util::rsa::encrypt_bytes_with_public_key(
                    &decrypted_with_bill_key,
                    &keys.public_key_pem,
                );
                self.bill_store
                    .save_attached_file(&encrypted, &bill_name, &file_name)
                    .await?;
                Ok(())
            }
        }
    }

    pub async fn get_key(&mut self, name: String) -> Vec<u8> {
        let providers = self.get_providers(name.clone()).await;
        if providers.is_empty() {
            error!("No providers was found.");
            Vec::new()
        } else {
            //TODO: If it's me - don't continue.
            let requests = providers.into_iter().map(|peer| {
                let mut network_client = self.clone();
                let local_peer_id = read_peer_id_from_file().to_string();

                let file_request = file_request_for_bill_keys(&local_peer_id, &name);
                async move { network_client.request_file(peer, file_request).await }.boxed()
            });

            let file_content = futures::future::select_ok(requests);

            let file_content_await = file_content.await;

            if file_content_await.is_err() {
                error!("None of the providers returned file.");
                Vec::new()
            } else {
                file_content_await
                    .map_err(|_| "None of the providers returned file.")
                    .expect("Can not get file content.")
                    .0
            }
        }
    }

    pub async fn put_bills_for_parties(&mut self) {
        let bills = get_bills().await;

        for bill in bills {
            let chain = Chain::read_chain_from_file(&bill.name);
            let nodes = chain.get_all_nodes_from_bill();
            for node in nodes {
                self.add_bill_to_dht_for_node(&bill.name, &node).await;
            }
        }
    }

    pub async fn subscribe_to_all_bills_topics(&mut self) {
        let bills = get_bills().await;

        for bill in bills {
            self.subscribe_to_topic(bill.name).await;
        }
    }

    pub async fn receive_updates_for_all_bills_topics(&mut self) {
        let bills = get_bills().await;

        for bill in bills {
            let event = GossipsubEvent::new(GossipsubEventId::CommandGetChain, vec![0; 24]);
            let message = event.to_byte_array();

            self.add_message_to_topic(message, bill.name).await;
        }
    }

    pub async fn subscribe_to_topic(&mut self, topic: String) {
        self.sender
            .send(Command::SubscribeToTopic { topic })
            .await
            .expect("Command receiver not to be dropped.");
    }

    async fn send_message(&mut self, msg: Vec<u8>, topic: String) {
        self.sender
            .send(Command::SendMessage { msg, topic })
            .await
            .expect("Command receiver not to be dropped.");
    }

    async fn put_record(&mut self, key: String, value: String) {
        self.sender
            .send(Command::PutRecord { key, value })
            .await
            .expect("Command receiver not to be dropped.");
    }

    async fn get_record(&mut self, key: String) -> Record {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::GetRecord { key, sender })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not to be dropped.")
    }

    async fn start_providing(&mut self, file_name: String) {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::StartProviding { file_name, sender })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not to be dropped.");
    }

    async fn get_providers(&mut self, file_name: String) -> HashSet<PeerId> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::GetProviders { file_name, sender })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not to be dropped.")
    }

    async fn request_file(&mut self, peer: PeerId, file_name: String) -> Result<Vec<u8>> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::RequestFile {
                file_name,
                peer,
                sender,
            })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not be dropped.")
    }

    async fn respond_file(&mut self, file: Vec<u8>, channel: ResponseChannel<FileResponse>) {
        self.sender
            .send(Command::RespondFile { file, channel })
            .await
            .expect("Command receiver not to be dropped.");
    }

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
                        let path_to_bill = get_path_for_bill(&bill_name);
                        match tokio::fs::read(&path_to_bill).await {
                            Err(e) => {
                                error!("Could not handle inbound request {request}: {e}")
                            }
                            Ok(file) => {
                                self.respond_file(file, channel).await;
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
                            let public_key = self
                                .get_identity_public_data_from_dht(node_id)
                                .await
                                .rsa_public_key_pem;

                            let path_to_key = get_path_for_bill_keys(&key_name);
                            match tokio::fs::read(&path_to_key).await {
                                Err(e) => {
                                    error!("Could not handle inbound request {request}: {e}")
                                }
                                Ok(file) => {
                                    let file_encrypted =
                                        encrypt_bytes_with_public_key(&file, &public_key);

                                    self.respond_file(file_encrypted, channel).await;
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
                            let public_key = self
                                .get_identity_public_data_from_dht(node_id)
                                .await
                                .rsa_public_key_pem;

                            match self
                                .bill_store
                                .open_attached_file(&bill_name, &file_name)
                                .await
                            {
                                Err(e) => {
                                    error!("Could not handle inbound request {request}: {e}")
                                }
                                Ok(file) => {
                                    let file_encrypted =
                                        encrypt_bytes_with_public_key(&file, &public_key);

                                    self.respond_file(file_encrypted, channel).await;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    //Need for testing from console.
    async fn handle_input_line(&mut self, line: String) {
        let mut args = line.split(' ');
        match args.next() {
            Some("PUT") => {
                let name: String = {
                    match args.next() {
                        Some(name) => String::from(name),
                        None => {
                            error!("Expected name.");
                            return;
                        }
                    }
                };
                self.put(&name).await;
            }

            Some("GET_BILL") => {
                let name: String = {
                    match args.next() {
                        Some(name) => String::from(name),
                        None => {
                            error!("Expected bill name.");
                            return;
                        }
                    }
                };
                self.get_bill(name).await;
            }

            Some("GET_BILL_ATTACHMENT") => {
                let name: String = {
                    match args.next() {
                        Some(name) => String::from(name),
                        None => {
                            error!("Expected bill name.");
                            return;
                        }
                    }
                };
                let file_name: String = {
                    match args.next() {
                        Some(file_name) => String::from(file_name),
                        None => {
                            error!("Expected file name.");
                            return;
                        }
                    }
                };
                if let Err(e) = self.get_bill_attachment(name, file_name).await {
                    error!("Get Bill Attachment failed: {e}");
                }
            }

            Some("GET_KEY") => {
                let name: String = {
                    match args.next() {
                        Some(name) => String::from(name),
                        None => {
                            error!("Expected bill name.");
                            return;
                        }
                    }
                };
                self.get_key(name).await;
            }

            Some("PUT_RECORD") => {
                let key = {
                    match args.next() {
                        Some(key) => String::from(key),
                        None => {
                            error!("Expected key");
                            return;
                        }
                    }
                };
                let value = {
                    match args.next() {
                        Some(value) => String::from(value),
                        None => {
                            error!("Expected value");
                            return;
                        }
                    }
                };

                self.put_record(key, value).await;
            }

            Some("SEND_MESSAGE") => {
                let topic = {
                    match args.next() {
                        Some(key) => String::from(key),
                        None => {
                            error!("Expected topic");
                            return;
                        }
                    }
                };
                let msg = {
                    match args.next() {
                        Some(value) => String::from(value),
                        None => {
                            error!("Expected msg");
                            return;
                        }
                    }
                };

                self.send_message(msg.into_bytes(), topic).await;
            }

            Some("SUBSCRIBE") => {
                let topic = {
                    match args.next() {
                        Some(key) => String::from(key),
                        None => {
                            error!("Expected topic");
                            return;
                        }
                    }
                };

                self.subscribe_to_topic(topic).await;
            }

            Some("GET_RECORD") => {
                let key = {
                    match args.next() {
                        Some(key) => String::from(key),
                        None => {
                            error!("Expected key");
                            return;
                        }
                    }
                };
                self.get_record(key).await;
            }

            Some("GET_PROVIDERS") => {
                let key = {
                    match args.next() {
                        Some(key) => String::from(key),
                        None => {
                            error!("Expected key");
                            return;
                        }
                    }
                };
                self.get_providers(key).await;
            }

            _ => {
                error!(
                        "expected GET_BILL, GET_KEY, GET_BILL_ATTACHMENT, PUT, SEND_MESSAGE, SUBSCRIBE, GET_RECORD, PUT_RECORD or GET_PROVIDERS."
                    );
            }
        }
    }
}
