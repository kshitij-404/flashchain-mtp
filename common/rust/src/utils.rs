use chrono::{DateTime, Utc};
use ethers::types::{U256, Address};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use hex::{FromHex, ToHex};

pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn format_address(address: &Address) -> String {
    format!("0x{}", hex::encode(address.as_bytes()))
}

pub fn parse_address(address: &str) -> Result<Address, hex::FromHexError> {
    if let Some(address) = address.strip_prefix("0x") {
        let bytes = Vec::from_hex(address)?;
        Ok(Address::from_slice(&bytes))
    } else {
        let bytes = Vec::from_hex(address)?;
        Ok(Address::from_slice(&bytes))
    }
}

pub fn format_amount(amount: U256) -> String {
    let ether = amount / U256::from(10).pow(U256::from(18));
    let remainder = amount % U256::from(10).pow(U256::from(18));
    format!("{}.{:018} ETH", ether, remainder)
}

pub fn timestamp_to_datetime(timestamp: u64) -> DateTime<Utc> {
    DateTime::from_utc(
        chrono::NaiveDateTime::from_timestamp_opt(timestamp as i64, 0).unwrap(),
        Utc,
    )
}

pub fn calculate_timeout(base_timeout: Duration, retry_count: u32) -> Duration {
    base_timeout * (2_u32.pow(retry_count))
}

pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_formatting() {
        let address = Address::random();
        let formatted = format_address(&address);
        let parsed = parse_address(&formatted).unwrap();
        assert_eq!(address, parsed);
    }

    #[test]
    fn test_amount_formatting() {
        let amount = U256::from(1234567890123456789u64);
        let formatted = format_amount(amount);
        assert!(formatted.contains("ETH"));
    }
}