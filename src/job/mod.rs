use crate::service::ServiceContext;
use crate::CONFIG;
use log::{error, info};
use std::time::Duration;
use tokio::{
    sync::broadcast,
    time::{interval, sleep},
};

pub async fn run(
    service_context: ServiceContext,
    mut shutdown_jobs_client_receiver: broadcast::Receiver<bool>,
) {
    sleep(Duration::from_secs(CONFIG.job_runner_initial_delay_seconds)).await;
    info!(
        "Job runner started after {}s of initial delay, running jobs every {}s...",
        CONFIG.job_runner_initial_delay_seconds, CONFIG.job_runner_check_interval_seconds
    );

    let mut check_interval_tick = interval(Duration::from_secs(
        CONFIG.job_runner_check_interval_seconds,
    ));

    loop {
        tokio::select! {
                _ = check_interval_tick.tick() => {
                    run_jobs(&service_context.clone()).await;
                },
                _ = shutdown_jobs_client_receiver.recv() => {
                    info!("Shutting down job runner...");
                    break;
                }
        }
    }
}

async fn run_jobs(service_context: &ServiceContext) {
    tokio::join!(
        run_check_bill_payment_job(service_context.clone()),
        run_check_bill_offer_to_sell_payment_job(service_context.clone())
    );
}

async fn run_check_bill_payment_job(service_context: ServiceContext) {
    info!("Running Check Bill Payment Job");
    if let Err(e) = service_context.bill_service.check_bills_payment().await {
        error!("Error while running Check Bill Payment Job: {e}");
    }
    info!("Finished running Check Bill Payment Job");
}

async fn run_check_bill_offer_to_sell_payment_job(service_context: ServiceContext) {
    info!("Running Check Bill Offer to Sell Payment Job");
    if let Err(e) = service_context
        .bill_service
        .check_bills_offer_to_sell_payment()
        .await
    {
        error!("Error while running Check Bill Offer to Sell Payment Job: {e}");
    }
    info!("Finished running Check Bill Offer to Sell Payment Job");
}
