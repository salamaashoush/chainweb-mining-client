//! Utility functions and helpers

pub mod logging;
pub mod memory;
pub mod monitoring;
pub mod units;

pub use logging::{LogContext, MiningMetrics, init_structured_logging};
pub use monitoring::{
    AlertConfig, HealthStatus, MonitoringSystem, PerformanceMetrics, global_monitoring,
    init_monitoring_with_pool,
};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_hashrate() {
        assert_eq!(format_hashrate(500), "500 H/s");
        assert_eq!(format_hashrate(1_500), "1.50 KH/s");
        assert_eq!(format_hashrate(2_500_000), "2.50 MH/s");
        assert_eq!(format_hashrate(3_500_000_000), "3.50 GH/s");
        assert_eq!(format_hashrate(4_500_000_000_000), "4.50 TH/s");
    }
}
