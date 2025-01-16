use crate::constants::DEFAULT_DATE_TIME_FORMAT;
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};

pub type DateTimeUtc = DateTime<Utc>;

/// Returns the current time as DateTime
#[allow(dead_code)]
pub fn now() -> DateTimeUtc {
    DateTime::default()
}

/// Quickly create a DateTimeUtc from a timestamp. chrone does not
/// really use Results and most of the errors are super unlikely to
/// happen.
pub fn seconds(timestamp: u64) -> DateTimeUtc {
    match Utc.timestamp_opt(timestamp as i64, 0).single() {
        Some(dt) => dt,
        None => panic!("invalid timestamp"),
    }
}

#[allow(dead_code)]
pub fn date_time_string_to_i64_timestamp(
    date_time_str: &str,
    format_str: Option<&str>,
) -> Option<i64> {
    let format = format_str.unwrap_or(DEFAULT_DATE_TIME_FORMAT);

    let naive_datetime = NaiveDateTime::parse_from_str(date_time_str, format).ok()?;
    let datetime_utc = Utc.from_utc_datetime(&naive_datetime);

    Some(datetime_utc.timestamp())
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
            date_time_string_to_i64_timestamp(date_time_str, None),
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
            date_time_string_to_i64_timestamp(date_time_str, Some(custom_format)),
            Some(expected_timestamp)
        );
    }

    #[test]
    fn test_date_time_string_to_u64_timestamp_with_invalid_date() {
        let date_time_str = "2025-13-40 00:00:00";
        assert_eq!(date_time_string_to_i64_timestamp(date_time_str, None), None);
    }

    #[test]
    fn test_date_time_string_to_u64_timestamp_with_invalid_format() {
        let date_time_str = "2025-01-15 00:00:00";
        let invalid_format = "%Q-%X-%Z";
        assert_eq!(
            date_time_string_to_i64_timestamp(date_time_str, Some(invalid_format)),
            None
        );
    }

    #[test]
    fn test_date_time_string_to_u64_timestamp_with_empty_string() {
        let date_time_str = "";
        assert_eq!(date_time_string_to_i64_timestamp(date_time_str, None), None);
    }

    #[test]
    fn test_date_time_string_to_u64_timestamp_with_custom_format_and_empty_string() {
        let date_time_str = "";
        let custom_format = "%d/%m/%Y %H/%M/%S";
        assert_eq!(
            date_time_string_to_i64_timestamp(date_time_str, Some(custom_format)),
            None
        );
    }
}
