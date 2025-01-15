use super::Result;
use crate::constants::DEFAULT_DATE_TIME_FORMAT;
use chrono::{NaiveDateTime, TimeZone, Utc};
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

    #[allow(dead_code)]
    pub fn date_time_string_to_u64_timestamp(
        date_time_str: &str,
        format_str: Option<&str>,
    ) -> Option<i64> {
        let format = match format_str {
            None => DEFAULT_DATE_TIME_FORMAT,
            _ => format_str.unwrap(),
        };

        let naive_datetime = NaiveDateTime::parse_from_str(date_time_str, format).ok()?;
        let datetime_utc = Utc.from_utc_datetime(&naive_datetime);

        Some(datetime_utc.timestamp())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_date_time_string_to_u64_timestamp_with_default_format() {
        let date_time_str = "2025-01-15 00:00:00";
        let expected_timestamp = Utc
            .with_ymd_and_hms(2025, 1, 15, 0, 0, 0)
            .unwrap()
            .timestamp();
        assert_eq!(
            TimeApi::date_time_string_to_u64_timestamp(date_time_str, None),
            Some(expected_timestamp)
        );
    }

    #[test]
    fn test_date_time_string_to_u64_timestamp_with_custom_format() {
        let date_time_str = "15/01/2025 12/30/45";
        let custom_format = "%d/%m/%Y %H/%M/%S";
        let expected_timestamp = Utc
            .with_ymd_and_hms(2025, 1, 15, 12, 30, 45)
            .unwrap()
            .timestamp();
        assert_eq!(
            TimeApi::date_time_string_to_u64_timestamp(date_time_str, Some(custom_format)),
            Some(expected_timestamp)
        );
    }

    #[test]
    fn test_date_time_string_to_u64_timestamp_with_invalid_date() {
        let date_time_str = "2025-13-40 00:00:00";
        assert_eq!(
            TimeApi::date_time_string_to_u64_timestamp(date_time_str, None),
            None
        );
    }

    #[test]
    fn test_date_time_string_to_u64_timestamp_with_invalid_format() {
        let date_time_str = "2025-01-15 00:00:00";
        let invalid_format = "%Q-%X-%Z";
        assert_eq!(
            TimeApi::date_time_string_to_u64_timestamp(date_time_str, Some(invalid_format)),
            None
        );
    }

    #[test]
    fn test_date_time_string_to_u64_timestamp_with_empty_string() {
        let date_time_str = "";
        assert_eq!(
            TimeApi::date_time_string_to_u64_timestamp(date_time_str, None),
            None
        );
    }

    #[test]
    fn test_date_time_string_to_u64_timestamp_with_custom_format_and_empty_string() {
        let date_time_str = "";
        let custom_format = "%d/%m/%Y %H/%M/%S";
        assert_eq!(
            TimeApi::date_time_string_to_u64_timestamp(date_time_str, Some(custom_format)),
            None
        );
    }
}
