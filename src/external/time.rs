use super::Result;
use chrono::Utc;
use log::error;
use serde::Deserialize;

/// Documented at https://timezonedb.com/references/get-time-zone
#[derive(Deserialize, Debug)]
pub struct TimeApi {
    pub timestamp: u64,
}

impl TimeApi {
    pub async fn get_atomic_time() -> Result<Self> {
        match reqwest::get("https://api.timezonedb.com/v2.1/get-time-zone?key=RQ6ZFDOXPVLR&format=json&by=zone&zone=Europe/Vienna")
            .await
            .map_err(super::Error::ExternalTimeApi)?
            .json()
            .await
            .map_err(super::Error::ExternalTimeApi) {
                Err(e) => {
                    // if there is an error with the API, fall back to local timestamp
                    error!("Error while fetching atomic time from API: {e}");
                    let utc_now = Utc::now();
                    let timestamp = utc_now.timestamp() as u64;
                    Ok(TimeApi {
                        timestamp
                    })
                },
                Ok(result) => Ok(result),
        }
    }
}
