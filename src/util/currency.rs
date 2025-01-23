use crate::service::{Error, Result};

pub fn parse_sum(sum: &str) -> Result<u64> {
    match sum.parse::<u64>() {
        Ok(num) => Ok(num),
        Err(_) => Err(Error::Validation(format!("invalid sum: {sum}"))),
    }
}

pub fn sum_to_string(sum: u64) -> String {
    sum.to_string()
}
