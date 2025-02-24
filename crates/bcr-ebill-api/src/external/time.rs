use crate::util;
use log::error;
use serde::Deserialize;

/// Documented at https://timezonedb.com/references/get-time-zone
#[derive(Deserialize, Debug)]
pub struct TimeApi {
    pub timestamp: u64,
}

impl TimeApi {
    pub async fn get_atomic_time() -> Self {
        match reqwest::get("https://vip.timezonedb.com/v2.1/get-time-zone?key=AY3Q7V1JPPNX&format=json&fields=timestamp&by=zone&zone=UTC")
            .await
            {
                Err(e) => {
                    handle_error(e)
                },
                Ok(result) => result.json().await.unwrap_or_else(|e| {
                    handle_error(e)
                }),
            }
    }
}

fn handle_error(e: reqwest::Error) -> TimeApi {
    // if there is an error with the API, fall back to local timestamp
    error!("Error while fetching atomic time from API: {e}");
    let utc_now = util::date::now();
    let timestamp = utc_now.timestamp() as u64;
    TimeApi { timestamp }
}
