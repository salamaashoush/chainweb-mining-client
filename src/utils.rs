//! Utility functions and helpers
//!
//! Common utilities used throughout the mining client.

use crate::{Error, Result};
use std::time::{SystemTime, UNIX_EPOCH};

/// Get current timestamp in microseconds since Unix epoch
pub fn current_timestamp_micros() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as i64
}

/// Get current timestamp in seconds since Unix epoch
pub fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Format bytes as a human-readable string
pub fn format_bytes(bytes: usize) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

/// Format hash rate as a human-readable string
pub fn format_hash_rate(hashes_per_sec: f64) -> String {
    const UNITS: &[&str] = &["H/s", "KH/s", "MH/s", "GH/s", "TH/s", "PH/s"];
    let mut rate = hashes_per_sec;
    let mut unit_index = 0;

    while rate >= 1000.0 && unit_index < UNITS.len() - 1 {
        rate /= 1000.0;
        unit_index += 1;
    }

    format!("{:.2} {}", rate, UNITS[unit_index])
}

/// Format duration as a human-readable string
pub fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else if seconds < 86400 {
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        let secs = seconds % 60;
        format!("{}h {}m {}s", hours, minutes, secs)
    } else {
        let days = seconds / 86400;
        let hours = (seconds % 86400) / 3600;
        format!("{}d {}h", days, hours)
    }
}

/// Validate hex string format
pub fn validate_hex_string(s: &str, expected_len: Option<usize>) -> Result<()> {
    if let Some(len) = expected_len {
        if s.len() != len {
            return Err(Error::generic(
                "hex validation",
                format!("Expected length {}, got {}", len, s.len()),
            ));
        }
    }

    if !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::generic(
            "hex validation", 
            "String contains non-hexadecimal characters"
        ));
    }

    Ok(())
}

/// Convert hex string to bytes
pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>> {
    validate_hex_string(hex, None)?;
    hex::decode(hex).map_err(|e| Error::generic("hex conversion", e.to_string()))
}

/// Convert bytes to hex string
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

/// Clamp a value between min and max
pub fn clamp<T: PartialOrd>(value: T, min: T, max: T) -> T {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// Calculate percentage
pub fn percentage(value: f64, total: f64) -> f64 {
    if total == 0.0 {
        0.0
    } else {
        (value / total) * 100.0
    }
}

/// Exponential backoff calculator
pub struct ExponentialBackoff {
    initial_delay_ms: u64,
    max_delay_ms: u64,
    multiplier: f64,
    current_attempt: u32,
}

impl ExponentialBackoff {
    /// Create a new exponential backoff calculator
    pub fn new(initial_delay_ms: u64, max_delay_ms: u64, multiplier: f64) -> Self {
        Self {
            initial_delay_ms,
            max_delay_ms,
            multiplier,
            current_attempt: 0,
        }
    }

    /// Get the next delay in milliseconds
    pub fn next_delay(&mut self) -> u64 {
        let delay = if self.current_attempt == 0 {
            self.initial_delay_ms
        } else {
            let exponential_delay = (self.initial_delay_ms as f64 
                * self.multiplier.powi(self.current_attempt as i32)) as u64;
            std::cmp::min(exponential_delay, self.max_delay_ms)
        };

        self.current_attempt += 1;
        delay
    }

    /// Reset the backoff state
    pub fn reset(&mut self) {
        self.current_attempt = 0;
    }

    /// Get current attempt number
    pub fn attempt(&self) -> u32 {
        self.current_attempt
    }
}

/// Rate limiter for controlling request frequency
#[derive(Debug)]
pub struct RateLimiter {
    last_request: std::time::Instant,
    min_interval: std::time::Duration,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(min_interval: std::time::Duration) -> Self {
        Self {
            last_request: std::time::Instant::now(),
            min_interval,
        }
    }

    /// Check if a request is allowed and update state
    pub fn try_request(&mut self) -> bool {
        let now = std::time::Instant::now();
        if now.duration_since(self.last_request) >= self.min_interval {
            self.last_request = now;
            true
        } else {
            false
        }
    }

    /// Wait until the next request is allowed
    pub async fn wait_for_request(&mut self) {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_request);
        
        if elapsed < self.min_interval {
            let wait_time = self.min_interval - elapsed;
            tokio::time::sleep(wait_time).await;
        }
        
        self.last_request = std::time::Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
    }

    #[test]
    fn test_format_hash_rate() {
        assert_eq!(format_hash_rate(100.0), "100.00 H/s");
        assert_eq!(format_hash_rate(1500.0), "1.50 KH/s");
        assert_eq!(format_hash_rate(1000000.0), "1.00 MH/s");
        assert_eq!(format_hash_rate(1500000000.0), "1.50 GH/s");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3661), "1h 1m 1s");
        assert_eq!(format_duration(90000), "1d 1h");
    }

    #[test]
    fn test_validate_hex_string() {
        assert!(validate_hex_string("deadbeef", Some(8)).is_ok());
        assert!(validate_hex_string("DEADBEEF", Some(8)).is_ok());
        assert!(validate_hex_string("123456789abcdef0", None).is_ok());
        
        assert!(validate_hex_string("deadbeef", Some(10)).is_err());
        assert!(validate_hex_string("deadbzzf", None).is_err());
        assert!(validate_hex_string("", Some(1)).is_err());
    }

    #[test]
    fn test_hex_conversion() {
        let bytes = vec![0xde, 0xad, 0xbe, 0xef];
        let hex = "deadbeef";
        
        assert_eq!(hex_to_bytes(hex).unwrap(), bytes);
        assert_eq!(bytes_to_hex(&bytes), hex);
    }

    #[test]
    fn test_clamp() {
        assert_eq!(clamp(5, 0, 10), 5);
        assert_eq!(clamp(-1, 0, 10), 0);
        assert_eq!(clamp(15, 0, 10), 10);
        assert_eq!(clamp(5.5, 0.0, 10.0), 5.5);
    }

    #[test]
    fn test_percentage() {
        assert_eq!(percentage(25.0, 100.0), 25.0);
        assert_eq!(percentage(1.0, 3.0), 100.0 / 3.0);
        assert_eq!(percentage(50.0, 0.0), 0.0);
    }

    #[test]
    fn test_exponential_backoff() {
        let mut backoff = ExponentialBackoff::new(100, 5000, 2.0);
        
        assert_eq!(backoff.next_delay(), 100);
        assert_eq!(backoff.next_delay(), 200);
        assert_eq!(backoff.next_delay(), 400);
        assert_eq!(backoff.next_delay(), 800);
        assert_eq!(backoff.next_delay(), 1600);
        assert_eq!(backoff.next_delay(), 3200);
        assert_eq!(backoff.next_delay(), 5000); // Capped at max
        
        assert_eq!(backoff.attempt(), 7);
        
        backoff.reset();
        assert_eq!(backoff.attempt(), 0);
        assert_eq!(backoff.next_delay(), 100); // Back to initial
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let mut limiter = RateLimiter::new(Duration::from_millis(100));
        
        // First request should be allowed immediately
        assert!(limiter.try_request());
        
        // Second request should be denied (too soon)
        assert!(!limiter.try_request());
        
        // Wait and try again
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(limiter.try_request());
    }

    #[tokio::test]
    async fn test_rate_limiter_wait() {
        let mut limiter = RateLimiter::new(Duration::from_millis(50));
        
        limiter.try_request(); // First request
        
        let start = std::time::Instant::now();
        limiter.wait_for_request().await; // Should wait ~50ms
        let elapsed = start.elapsed();
        
        assert!(elapsed >= Duration::from_millis(45)); // Allow some tolerance
        assert!(elapsed <= Duration::from_millis(100)); // Upper bound
    }

    #[test]
    fn test_current_timestamp() {
        let ts_secs = current_timestamp_secs();
        let ts_micros = current_timestamp_micros();
        
        // Sanity checks - should be reasonable values
        assert!(ts_secs > 1_600_000_000); // After 2020
        assert!(ts_secs < 2_000_000_000); // Before 2033
        
        assert!(ts_micros > 1_600_000_000_000_000); // After 2020 in microseconds
        assert!(ts_micros as u64 / 1_000_000 <= ts_secs + 1); // Should be consistent
    }
}