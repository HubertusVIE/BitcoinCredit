use super::Result;
use serde::Deserialize;

/// Documented at https://timezonedb.com/references/get-time-zone
#[derive(Deserialize, Debug)]
pub struct TimeApi {
    pub timestamp: i64,
}

#[cfg(not(test))]
impl TimeApi {
    pub async fn get_atomic_time() -> Result<Self> {
        reqwest::get("https://api.timezonedb.com/v2.1/get-time-zone?key=RQ6ZFDOXPVLR&format=json&by=zone&zone=Europe/Vienna")
            .await
            .map_err(super::Error::ExternalTimeApi)?
            .json()
            .await
            .map_err(super::Error::ExternalTimeApi)
    }
}

#[cfg(test)]
impl TimeApi {
    pub async fn get_atomic_time() -> Result<Self> {
        Ok(TimeApi {
            timestamp: 1731593928,
        })
    }
}
