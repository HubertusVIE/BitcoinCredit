use anyhow::Result;
use bcr_ebill_api::get_db_context;
use bcr_ebill_api::service::create_service_context;
use clap::Parser;
use config::Config;
use constants::SHUTDOWN_GRACE_PERIOD_MS;
use log::{error, info};
use tokio::spawn;

mod api_docs;
mod config;
mod constants;
mod data;
mod error;
mod handlers;
mod job;
mod router;

// MAIN
#[macro_use]
extern crate lazy_static;
lazy_static! {
    pub static ref CONFIG: Config = Config::parse();
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let conf = CONFIG.clone();
    // Initialize the API
    let api_config = bcr_ebill_api::Config {
        bitcoin_network: conf.bitcoin_network.clone(),
        nostr_relay: conf.nostr_relay.clone(),
        relay_bootstrap_address: conf.relay_bootstrap_address.clone(),
        relay_bootstrap_peer_id: conf.relay_bootstrap_peer_id.clone(),
        surreal_db_connection: conf.surreal_db_connection.clone(),
        data_dir: conf.data_dir.clone(),
        p2p_address: conf.p2p_address.clone(),
        p2p_port: conf.p2p_port,
    };
    info!("Chosen Network: {:?}", api_config.bitcoin_network());
    bcr_ebill_api::init(api_config.clone())?;

    loop {
        let (reboot_sender, mut reboot_receiver) = tokio::sync::watch::channel(false);
        if let Err(e) = start(api_config.clone(), reboot_sender).await {
            error!("Error starting the application: {e}");
            break;
        }

        let reboot = *reboot_receiver.borrow_and_update();
        if reboot {
            // we need to give the os time to finish its disk operations
            tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
            info!("Restarting application...");
        } else {
            break;
        }
    }

    Ok(())
}

async fn start(
    api_config: bcr_ebill_api::Config,
    reboot_sender: tokio::sync::watch::Sender<bool>,
) -> Result<()> {
    // Initialize the database context
    let db = get_db_context(&api_config).await?;

    let dht = bcr_ebill_api::dht_main(
        &api_config,
        db.bill_store.clone(),
        db.bill_blockchain_store.clone(),
        db.company_store.clone(),
        db.company_chain_store.clone(),
        db.identity_store.clone(),
        db.file_upload_store.clone(),
    )
    .await
    .expect("DHT failed to start");
    let dht_client = dht.client;

    let ctrl_c_sender = dht.shutdown_sender.clone();
    spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("can't register ctrl-c handler");
        info!("Received SIGINT. Shutting down...");

        if let Err(e) = ctrl_c_sender.send(true) {
            error!("Error triggering shutdown signal: {e}");
        }
    });

    if CONFIG.terminal_client {
        let terminal_client_shutdown_receiver = dht.shutdown_sender.clone().subscribe();
        let terminal_dht_client = dht_client.clone();
        spawn(bcr_ebill_api::util::terminal::run_terminal_client(
            terminal_client_shutdown_receiver,
            terminal_dht_client,
        ));
    }

    let local_node_id = db.identity_store.get_key_pair().await?.get_public_key();
    let mut dht_client_clone = dht_client.clone();
    let identity_store_clone = db.identity_store.clone();
    spawn(async move {
        // These actions only make sense, if we already have created an identity
        // We do them asynchronously, in a non-failing way
        if identity_store_clone.exists().await {
            if let Err(e) = dht_client_clone.check_new_bills().await {
                error!("Error while checking for new bills: {e}");
            }

            if let Err(e) = dht_client_clone.subscribe_to_all_bills_topics().await {
                error!("Error while subscribing to bills: {e}");
            }

            if let Err(e) = dht_client_clone.put_bills_for_parties().await {
                error!("Error while putting bills for parties: {e}");
            }

            if let Err(e) = dht_client_clone.start_providing_bills().await {
                error!("Error while starting to provide bills: {e}");
            }

            if let Err(e) = dht_client_clone
                .receive_updates_for_all_bills_topics()
                .await
            {
                error!("Error while starting receive updates for bill topics: {e}");
            }

            if let Err(e) = dht_client_clone.check_companies().await {
                error!("Error while checking for new companies: {e}");
            }

            if let Err(e) = dht_client_clone.put_companies_for_signatories().await {
                error!("Error while putting companies for signatories: {e}");
            }

            if let Err(e) = dht_client_clone.start_providing_companies().await {
                error!("Error while starting to provide companies: {e}");
            }

            if let Err(e) = dht_client_clone.subscribe_to_all_companies_topics().await {
                error!("Error while subscribing to all companies: {e}");
            }
        }
    });

    let job_shutdown_receiver = dht.shutdown_sender.clone().subscribe();
    let web_server_error_shutdown_sender = dht.shutdown_sender.clone();
    let service_context = create_service_context(
        &local_node_id,
        api_config.clone(),
        dht_client.clone(),
        dht.shutdown_sender,
        db,
        reboot_sender,
    )
    .await?;

    let service_context_clone = service_context.clone();
    spawn(async move { job::run(service_context_clone, job_shutdown_receiver).await });

    let nostr_handle = service_context.nostr_consumer.start().await?;

    if let Err(e) = router::rocket_main(CONFIG.clone(), service_context)
        .launch()
        .await
    {
        error!("Web server stopped with error: {e}, shutting down the rest of the application...");
        if let Err(e) = web_server_error_shutdown_sender.send(true) {
            error!("Error triggering shutdown signal: {e}");
        }
    }

    info!("Stopping nostr consumer...");
    nostr_handle.abort();

    info!("Waiting for application to exit...");
    // If the web server exits fast, we wait for a grace period so libp2p can finish as well
    tokio::time::sleep(std::time::Duration::from_millis(SHUTDOWN_GRACE_PERIOD_MS)).await;

    Ok(())
}
