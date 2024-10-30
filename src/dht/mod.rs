use crate::bill::identity::{
    generate_dht_logic, read_ed25519_keypair_from_file, read_peer_id_from_file,
};
use crate::config::Config;
use crate::constants::{
    IDENTITY_ED_25529_KEYS_FILE_PATH, IDENTITY_PEER_ID_FILE_PATH, RELAY_BOOTSTRAP_NODE_ONE_IP,
    RELAY_BOOTSTRAP_NODE_ONE_PEER_ID, RELAY_BOOTSTRAP_NODE_ONE_TCP,
};
use behaviour::{ComposedEvent, Event, MyBehaviour};
use event_loop::EventLoop;
use futures::channel::mpsc;
use futures::channel::mpsc::Receiver;
use futures::prelude::*;
use libp2p::core::transport::OrTransport;
use libp2p::core::upgrade::Version;
use libp2p::core::Multiaddr;
use libp2p::dns::DnsConfig;
use libp2p::multiaddr::Protocol;
use libp2p::multihash::Multihash;
use libp2p::swarm::{SwarmBuilder, SwarmEvent};
use libp2p::{identify, noise, relay, tcp, yamux, PeerId, Transport};
use std::error::Error;
use std::path::Path;
use tokio::{spawn, sync::broadcast};

mod behaviour;
mod client;
mod event_loop;

pub use client::Client;
use log::{error, info};

#[derive(Debug)]
pub struct Dht {
    pub client: Client,
    pub shutdown_sender: broadcast::Sender<bool>,
}

pub async fn dht_main(conf: &Config) -> Result<Dht, Box<dyn Error + Send + Sync>> {
    let (network_client, network_events, network_event_loop) = new(conf)
        .await
        .expect("Can not to create network module in dht.");

    let (shutdown_sender, shutdown_receiver) = broadcast::channel::<bool>(100);

    spawn(network_event_loop.run(shutdown_receiver));

    let network_client_to_return = network_client.clone();

    spawn(network_client.run(network_events, shutdown_sender.subscribe()));

    Ok(Dht {
        client: network_client_to_return,
        shutdown_sender,
    })
}

async fn new(
    conf: &Config,
) -> Result<(Client, Receiver<Event>, event_loop::EventLoop), Box<dyn Error>> {
    if !Path::new(IDENTITY_PEER_ID_FILE_PATH).exists()
        && !Path::new(IDENTITY_ED_25529_KEYS_FILE_PATH).exists()
    {
        generate_dht_logic();
    }

    let local_public_key = read_ed25519_keypair_from_file();
    let local_peer_id = read_peer_id_from_file();
    info!("Local peer id: {local_peer_id:?}");

    let (relay_transport, client) = relay::client::new(local_peer_id);

    let dns_cfg = DnsConfig::system(tcp::tokio::Transport::new(
        tcp::Config::default().port_reuse(true),
    ))
    .await;
    let transport = OrTransport::new(relay_transport, dns_cfg.unwrap())
        .upgrade(Version::V1Lazy)
        .authenticate(noise::Config::new(&local_public_key).unwrap())
        .multiplex(yamux::Config::default())
        .timeout(std::time::Duration::from_secs(20))
        .boxed();

    let behaviour = MyBehaviour::new(local_peer_id, local_public_key.clone(), client);

    let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();

    swarm.listen_on(conf.p2p_listen_url().unwrap()).unwrap();

    // Wait to listen on all interfaces.
    let sleep = tokio::time::sleep(std::time::Duration::from_secs(1));
    tokio::pin!(sleep);

    loop {
        tokio::select! {
            event = swarm.next() => {
                match event.unwrap() {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!("Listening on {:?}", address);
                    }
                    SwarmEvent::Behaviour { .. } => {
                    }
                    event => panic!("{event:?}"),
                }
            }
            _ = &mut sleep => {
                // Likely listening on all interfaces now, thus continuing by breaking the loop.
                break;
            }
        }
    }

    let relay_peer_id: PeerId = RELAY_BOOTSTRAP_NODE_ONE_PEER_ID
        .to_string()
        .parse()
        .expect("Can not to parse relay peer id.");
    let relay_address = Multiaddr::empty()
        .with(Protocol::Ip4(RELAY_BOOTSTRAP_NODE_ONE_IP))
        .with(Protocol::Tcp(RELAY_BOOTSTRAP_NODE_ONE_TCP))
        .with(Protocol::P2p(Multihash::from(relay_peer_id)));
    info!("Relay address: {:?}", relay_address);

    swarm.dial(relay_address.clone()).unwrap();
    let mut learned_observed_addr = false;
    let mut told_relay_observed_addr = false;

    loop {
        match swarm.next().await.unwrap() {
            SwarmEvent::NewListenAddr { .. } => {}
            SwarmEvent::Dialing { .. } => {}
            SwarmEvent::ConnectionEstablished { .. } => {}
            SwarmEvent::Behaviour(ComposedEvent::Identify(identify::Event::Sent { .. })) => {
                info!("Told relay its public address.");
                told_relay_observed_addr = true;
            }
            SwarmEvent::Behaviour(ComposedEvent::Identify(identify::Event::Received {
                info: identify::Info { observed_addr, .. },
                ..
            })) => {
                info!("Relay told us our public address: {:?}", observed_addr);
                learned_observed_addr = true;
            }
            SwarmEvent::Behaviour { .. } => {}
            event => panic!("{event:?}"),
        }

        if learned_observed_addr && told_relay_observed_addr {
            break;
        }
    }

    swarm.behaviour_mut().bootstrap_kademlia();

    swarm
        .listen_on(relay_address.clone().with(Protocol::P2pCircuit))
        .unwrap();

    loop {
        match swarm.next().await.unwrap() {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {:?}", address);
                break;
            }
            SwarmEvent::Behaviour(ComposedEvent::Relay(
                relay::client::Event::ReservationReqAccepted { .. },
            )) => {
                info!("Relay accepted our reservation request.");
            }
            SwarmEvent::Behaviour(ComposedEvent::Relay(event)) => {
                info!("Relay event: {:?}", event)
            }
            SwarmEvent::Behaviour(ComposedEvent::Dcutr(event)) => {
                info!("Dcutr event: {:?}", event)
            }
            SwarmEvent::Behaviour(ComposedEvent::Identify(event)) => {
                info!("Identify event: {:?}", event)
            }
            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                info!("Established connection to {:?} via {:?}", peer_id, endpoint);
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error } => {
                error!("Outgoing connection error to {:?}: {:?}", peer_id, error);
            }
            SwarmEvent::Behaviour(event) => {
                info!("Behaviour event: {event:?}")
            }
            _ => {}
        }
    }

    let (command_sender, command_receiver) = mpsc::channel(0);
    let (event_sender, event_receiver) = mpsc::channel(0);
    let event_loop = EventLoop::new(swarm, command_receiver, event_sender);

    Ok((Client::new(command_sender), event_receiver, event_loop))
}
