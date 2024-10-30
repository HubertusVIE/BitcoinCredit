use crate::constants::{
    BILLS_FOLDER_PATH, BILLS_KEYS_FOLDER_PATH, BOOTSTRAP_FOLDER_PATH, IDENTITY_FOLDER_PATH,
    QUOTES_MAP_FOLDER_PATH,
};
use anyhow::Result;
use clap::Parser;
use config::Config;
use log::info;
use service::create_service_context;
use std::path::Path;
use std::{env, fs};

mod bill;
mod blockchain;
mod config;
mod constants;
mod dht;
mod external;
mod persistence;
mod service;
#[cfg(test)]
mod tests;
mod util;
mod web;

// MAIN
#[tokio::main]
async fn main() -> Result<()> {
    env::set_var("RUST_BACKTRACE", "full");

    env_logger::init();

    // Parse command line arguments and env vars with clap
    let conf = Config::parse();

    init_folders();

    external::mint::init_wallet().await;

    let dht = dht::dht_main(&conf).await.expect("DHT failed to start");
    let mut shutdown_receiver = dht.shutdown_sender.subscribe();
    let mut dht_client = dht.client;

    let local_peer_id = bill::identity::read_peer_id_from_file();
    dht_client
        .check_new_bills(local_peer_id.to_string().clone())
        .await;
    dht_client
        .upgrade_table(local_peer_id.to_string().clone())
        .await;
    dht_client.subscribe_to_all_bills_topics().await;
    dht_client.put_bills_for_parties().await;
    dht_client.start_provide().await;
    dht_client.receive_updates_for_all_bills_topics().await;
    dht_client.put_identity_public_data_in_dht().await;
    let service_context =
        create_service_context(conf.clone(), dht_client.clone(), dht.shutdown_sender).await?;
    let _rocket = web::rocket_main(service_context).launch().await?;

    info!("web server was shut down...");
    // Wait for shutdown event after rocket server stopped
    // TODO: race with timeout
    shutdown_receiver
        .recv()
        .await
        .expect("error during shutdown");

    info!("shutdown event received");
    // TODO: create ctrl-c handler
    // TODO: sleep for a while, then stop
    // std::process::exit(0x0100);
    Ok(())
}

fn init_folders() {
    if !Path::new(QUOTES_MAP_FOLDER_PATH).exists() {
        fs::create_dir(QUOTES_MAP_FOLDER_PATH).expect("Can't create folder quotes.");
    }
    if !Path::new(IDENTITY_FOLDER_PATH).exists() {
        fs::create_dir(IDENTITY_FOLDER_PATH).expect("Can't create folder identity.");
    }
    if !Path::new(BILLS_FOLDER_PATH).exists() {
        fs::create_dir(BILLS_FOLDER_PATH).expect("Can't create folder bills.");
    }
    if !Path::new(BILLS_KEYS_FOLDER_PATH).exists() {
        fs::create_dir(BILLS_KEYS_FOLDER_PATH).expect("Can't create folder bills_keys.");
    }
    if !Path::new(BOOTSTRAP_FOLDER_PATH).exists() {
        fs::create_dir(BOOTSTRAP_FOLDER_PATH).expect("Can't create folder bootstrap.");
    }
}
