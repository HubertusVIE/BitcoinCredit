use chrono::{DateTime, TimeZone, Utc};

pub type DateTimeUtc = DateTime<Utc>;

/// Returns the current time as DateTime
#[allow(dead_code)]
pub fn now() -> DateTimeUtc {
    DateTime::default()
}

/// Quickly create a DateTimeUtc from a timestamp. chrone does not
/// really use Results and most of the errors are super unlikely to
/// happen.
#[allow(dead_code)]
pub fn seconds(timestamp: i64) -> DateTimeUtc {
    match Utc.timestamp_opt(timestamp, 0).single() {
        Some(dt) => dt,
        None => panic!("invalid timestamp"),
    }
}

/// Nostr timestamps are unsigned 64 bit integers. This function converts
/// them to a DateTimeUtc which we can directly use in SurrealDB and
/// everywhere else.
#[allow(dead_code)]
pub fn seconds_unsigned(timestamp: u64) -> DateTimeUtc {
    seconds(timestamp as i64)
}
