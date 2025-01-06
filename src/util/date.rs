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
pub fn seconds(timestamp: u64) -> DateTimeUtc {
    match Utc.timestamp_opt(timestamp as i64, 0).single() {
        Some(dt) => dt,
        None => panic!("invalid timestamp"),
    }
}
