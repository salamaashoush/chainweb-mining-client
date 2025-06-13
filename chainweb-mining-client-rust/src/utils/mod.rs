//! Utility functions and helpers

use std::time::{SystemTime, UNIX_EPOCH};
use tracing_subscriber::EnvFilter;

/// Initialize logging based on configuration
pub fn init_logging(level: &str, format: &str) {
    let env_filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("info"));

    match format {
        "json" => {
            tracing_subscriber::fmt()
                .json()
                .with_env_filter(env_filter)
                .with_target(false)
                .init();
        }
        _ => {
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_target(false)
                .init();
        }
    }
}

/// Get current timestamp in seconds
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

/// Format hashrate for display
pub fn format_hashrate(hashrate: u64) -> String {
    if hashrate >= 1_000_000_000_000 {
        format!("{:.2} TH/s", hashrate as f64 / 1_000_000_000_000.0)
    } else if hashrate >= 1_000_000_000 {
        format!("{:.2} GH/s", hashrate as f64 / 1_000_000_000.0)
    } else if hashrate >= 1_000_000 {
        format!("{:.2} MH/s", hashrate as f64 / 1_000_000.0)
    } else if hashrate >= 1_000 {
        format!("{:.2} KH/s", hashrate as f64 / 1_000.0)
    } else {
        format!("{} H/s", hashrate)
    }
}

/// Convert difficulty to target bytes (simplified)
pub fn difficulty_to_target(difficulty: f64) -> [u8; 32] {
    // This is a simplified conversion
    // In practice, you'd need the exact formula used by Chainweb
    let max_target = u64::MAX as f64;
    let target_value = (max_target / difficulty) as u64;

    let mut bytes = [0xFFu8; 32];
    bytes[24..32].copy_from_slice(&target_value.to_be_bytes());

    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_timestamp() {
        let ts1 = current_timestamp();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let ts2 = current_timestamp();
        assert!(ts2 >= ts1);
    }

    #[test]
    fn test_format_hashrate() {
        assert_eq!(format_hashrate(500), "500 H/s");
        assert_eq!(format_hashrate(1_500), "1.50 KH/s");
        assert_eq!(format_hashrate(2_500_000), "2.50 MH/s");
        assert_eq!(format_hashrate(3_500_000_000), "3.50 GH/s");
        assert_eq!(format_hashrate(4_500_000_000_000), "4.50 TH/s");
    }

    #[test]
    fn test_difficulty_to_target() {
        let target = difficulty_to_target(1.0);
        assert_eq!(target[0..24], [0xFF; 24]);

        let harder_target = difficulty_to_target(2.0);
        // Should be roughly half the value
        let value1 = u64::from_be_bytes(target[24..32].try_into().unwrap());
        let value2 = u64::from_be_bytes(harder_target[24..32].try_into().unwrap());
        assert!(value2 < value1);
    }
}
