use crate::constants::{
    BILL_ATTACHMENT_PREFIX, BILL_PREFIX, BOOTSTRAP_NODES_FILE_PATH, KEY_PREFIX, MAX_FILE_SIZE_BYTES,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::channel::oneshot;
use futures::prelude::*;
use libp2p::core::upgrade::{read_length_prefixed, write_length_prefixed, ProtocolName};
use libp2p::identity::Keypair;
use libp2p::kad::record::store::MemoryStore;
use libp2p::kad::record::Record;
use libp2p::kad::{Kademlia, KademliaEvent};
use libp2p::request_response::{self, ProtocolSupport, ResponseChannel};
use libp2p::swarm::NetworkBehaviour;
use libp2p::{dcutr, gossipsub, identify, relay, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::{fs, iter};

#[derive(Deserialize, Serialize, Debug)]
struct Nodes {
    node: String,
    address: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct NodesJson {
    nodes: Vec<Nodes>,
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ComposedEvent", event_process = false)]
pub struct MyBehaviour {
    pub request_response: request_response::Behaviour<FileExchangeCodec>,
    pub kademlia: Kademlia<MemoryStore>,
    pub identify: identify::Behaviour,
    pub gossipsub: gossipsub::Behaviour,
    pub relay_client: relay::client::Behaviour,
    pub dcutr: dcutr::Behaviour,
}

impl MyBehaviour {
    pub fn new(
        local_peer_id: PeerId,
        local_public_key: Keypair,
        client: relay::client::Behaviour,
    ) -> Self {
        Self {
            request_response: {
                request_response::Behaviour::new(
                    FileExchangeCodec(),
                    iter::once((FileExchangeProtocol(), ProtocolSupport::Full)),
                    Default::default(),
                )
            },
            kademlia: {
                let store = MemoryStore::new(local_peer_id);
                Kademlia::new(local_peer_id, store)
            },
            identify: {
                let cfg_identify =
                    identify::Config::new("/identify/0.1.0".to_string(), local_public_key.public());
                identify::Behaviour::new(cfg_identify)
            },
            gossipsub: {
                let gossipsub_config = libp2p::gossipsub::Config::default();
                let message_authenticity =
                    gossipsub::MessageAuthenticity::Signed(local_public_key.clone());
                gossipsub::Behaviour::new(message_authenticity, gossipsub_config)
                    .expect("Correct configuration")
            },
            relay_client: { client },
            dcutr: { dcutr::Behaviour::new(local_peer_id) },
        }
    }

    pub fn bootstrap_kademlia(&mut self) {
        let boot_nodes_string = fs::read_to_string(BOOTSTRAP_NODES_FILE_PATH)
            .expect("Can't read bootstrap nodes file.");
        let boot_nodes = serde_json::from_str::<NodesJson>(&boot_nodes_string)
            .expect("Can't parse bootstrap nodes file.");
        for index in 0..boot_nodes.nodes.len() {
            let node = boot_nodes.nodes[index].node.clone();
            let address = boot_nodes.nodes[index].address.clone();
            self.kademlia.add_address(
                &node.parse().expect("Can't parse bootstrap node id"),
                address.parse().expect("Can't parse bootstrap node address"),
            );
        }
        self.kademlia.bootstrap().expect("Cant bootstrap");
    }
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ComposedEvent {
    RequestResponse(request_response::Event<FileRequest, FileResponse>),
    Kademlia(KademliaEvent),
    Identify(identify::Event),
    Gossipsub(gossipsub::Event),
    Relay(relay::client::Event),
    Dcutr(dcutr::Event),
}

impl From<request_response::Event<FileRequest, FileResponse>> for ComposedEvent {
    fn from(event: request_response::Event<FileRequest, FileResponse>) -> Self {
        ComposedEvent::RequestResponse(event)
    }
}

impl From<KademliaEvent> for ComposedEvent {
    fn from(event: KademliaEvent) -> Self {
        ComposedEvent::Kademlia(event)
    }
}

impl From<identify::Event> for ComposedEvent {
    fn from(event: identify::Event) -> Self {
        ComposedEvent::Identify(event)
    }
}

impl From<gossipsub::Event> for ComposedEvent {
    fn from(event: gossipsub::Event) -> Self {
        ComposedEvent::Gossipsub(event)
    }
}

impl From<relay::client::Event> for ComposedEvent {
    fn from(event: relay::client::Event) -> Self {
        ComposedEvent::Relay(event)
    }
}

impl From<dcutr::Event> for ComposedEvent {
    fn from(event: dcutr::Event) -> Self {
        ComposedEvent::Dcutr(event)
    }
}

#[derive(Debug)]
pub enum Command {
    StartProviding {
        file_name: String,
        sender: oneshot::Sender<()>,
    },
    GetProviders {
        file_name: String,
        sender: oneshot::Sender<HashSet<PeerId>>,
    },
    PutRecord {
        key: String,
        value: String,
    },
    GetRecord {
        key: String,
        sender: oneshot::Sender<Record>,
    },
    RequestFile {
        file_name: String,
        peer: PeerId,
        sender: oneshot::Sender<Result<Vec<u8>>>,
    },
    RespondFile {
        file: Vec<u8>,
        channel: ResponseChannel<FileResponse>,
    },
    SendMessage {
        msg: Vec<u8>,
        topic: String,
    },
    SubscribeToTopic {
        topic: String,
    },
}

#[derive(Debug)]
pub enum Event {
    InboundRequest {
        request: String,
        channel: ResponseChannel<FileResponse>,
    },
}

#[derive(Debug, Clone)]
pub enum ParsedInboundFileRequest {
    Bill(BillFileRequest),
    BillKeys(BillKeysFileRequest),
    BillAttachment(BillAttachmentFileRequest),
}

#[derive(Debug, Clone)]
pub struct BillFileRequest {
    pub bill_name: String,
}

#[derive(Debug, Clone)]
pub struct BillKeysFileRequest {
    pub node_id: String,
    pub key_name: String,
}

#[derive(Debug, Clone)]
pub struct BillAttachmentFileRequest {
    pub node_id: String,
    pub bill_name: String,
    pub file_name: String,
}

pub fn file_request_for_bill_attachment(node_id: &str, bill_name: &str, file_name: &str) -> String {
    format!("{node_id}_{BILL_ATTACHMENT_PREFIX}_{bill_name}_{file_name}")
}

pub fn parse_inbound_file_request(request: &str) -> Result<ParsedInboundFileRequest> {
    let parts = request.split("_").collect::<Vec<&str>>();
    if parts.len() < 3 {
        return Err(anyhow!("invalid file request {request}"));
    }

    let node_id = parts[0].to_owned();
    let prefix = parts[1];
    match prefix {
        BILL_PREFIX => Ok(ParsedInboundFileRequest::Bill(BillFileRequest {
            bill_name: parts[2].to_owned(),
        })),
        KEY_PREFIX => Ok(ParsedInboundFileRequest::BillKeys(BillKeysFileRequest {
            node_id,
            key_name: parts[2].to_owned(),
        })),
        BILL_ATTACHMENT_PREFIX => {
            if parts.len() < 4 {
                return Err(anyhow!("invalid file request {request}"));
            }
            Ok(ParsedInboundFileRequest::BillAttachment(
                BillAttachmentFileRequest {
                    node_id,
                    bill_name: parts[2].to_owned(),
                    file_name: parts[3].to_owned(),
                },
            ))
        }
        _ => Err(anyhow!("invalid file request {request}")),
    }
}

#[derive(Debug, Clone)]
pub struct FileExchangeProtocol();

#[derive(Clone)]
pub struct FileExchangeCodec();

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRequest(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileResponse(pub Vec<u8>);

impl ProtocolName for FileExchangeProtocol {
    fn protocol_name(&self) -> &[u8] {
        "/file-exchange/0.1.0".as_bytes()
    }
}

#[async_trait]
impl request_response::Codec for FileExchangeCodec {
    type Protocol = FileExchangeProtocol;
    type Request = FileRequest;
    type Response = FileResponse;

    async fn read_request<T>(
        &mut self,
        _: &FileExchangeProtocol,
        io: &mut T,
    ) -> tokio::io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, MAX_FILE_SIZE_BYTES).await?;

        if vec.is_empty() {
            return Err(tokio::io::ErrorKind::UnexpectedEof.into());
        }

        Ok(FileRequest(
            String::from_utf8(vec).map_err(|_| tokio::io::ErrorKind::InvalidData)?,
        ))
    }

    async fn read_response<T>(
        &mut self,
        _: &FileExchangeProtocol,
        io: &mut T,
    ) -> tokio::io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, MAX_FILE_SIZE_BYTES).await?;

        if vec.is_empty() {
            return Err(tokio::io::ErrorKind::UnexpectedEof.into());
        }

        Ok(FileResponse(vec))
    }

    async fn write_request<T>(
        &mut self,
        _: &FileExchangeProtocol,
        io: &mut T,
        FileRequest(data): FileRequest,
    ) -> tokio::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, data).await?;
        io.close().await?;

        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _: &FileExchangeProtocol,
        io: &mut T,
        FileResponse(data): FileResponse,
    ) -> tokio::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, data).await?;
        io.close().await?;

        Ok(())
    }
}
