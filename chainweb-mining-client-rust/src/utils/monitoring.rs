//! Production-grade monitoring and alerting framework
//!
//! This module provides comprehensive monitoring capabilities for production
//! deployments, including metrics collection, health checks, and alerting.

use crate::protocol::http_pool::HttpClientPool;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};

/// System health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// System is operating normally
    Healthy,
    /// System has minor issues but is functional
    Warning,
    /// System has major issues affecting operation
    Critical,
    /// System is down or unresponsive
    Down,
}

/// Performance metrics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Current hash rate (hashes per second)
    pub hash_rate: f64,
    /// Average hash rate over last period
    pub avg_hash_rate: f64,
    /// Peak hash rate observed
    pub peak_hash_rate: f64,
    /// Number of solutions found
    pub solutions_found: u64,
    /// Number of shares submitted
    pub shares_submitted: u64,
    /// Share acceptance rate (0.0 to 1.0)
    pub acceptance_rate: f64,
    /// Average response time for work requests (milliseconds)
    pub avg_response_time_ms: f64,
    /// Memory usage in bytes
    pub memory_usage_bytes: u64,
    /// CPU utilization percentage (0.0 to 100.0)
    pub cpu_utilization: f64,
    /// Uptime in seconds
    pub uptime_seconds: u64,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            hash_rate: 0.0,
            avg_hash_rate: 0.0,
            peak_hash_rate: 0.0,
            solutions_found: 0,
            shares_submitted: 0,
            acceptance_rate: 0.0,
            avg_response_time_ms: 0.0,
            memory_usage_bytes: 0,
            cpu_utilization: 0.0,
            uptime_seconds: 0,
        }
    }
}

/// Alert configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    /// Minimum hash rate before alerting
    pub min_hash_rate: f64,
    /// Maximum response time before alerting (milliseconds)
    pub max_response_time_ms: f64,
    /// Minimum acceptance rate before alerting (0.0 to 1.0)
    pub min_acceptance_rate: f64,
    /// Maximum memory usage before alerting (bytes)
    pub max_memory_usage_bytes: u64,
    /// Maximum CPU utilization before alerting (0.0 to 100.0)
    pub max_cpu_utilization: f64,
    /// Enable/disable specific alert types
    pub enabled_alerts: HashMap<String, bool>,
}

impl Default for AlertConfig {
    fn default() -> Self {
        let mut enabled_alerts = HashMap::new();
        enabled_alerts.insert("hash_rate".to_string(), true);
        enabled_alerts.insert("response_time".to_string(), true);
        enabled_alerts.insert("acceptance_rate".to_string(), true);
        enabled_alerts.insert("memory_usage".to_string(), true);
        enabled_alerts.insert("cpu_usage".to_string(), true);
        enabled_alerts.insert("connection_issues".to_string(), true);

        Self {
            min_hash_rate: 1000.0,                      // 1 KH/s minimum
            max_response_time_ms: 10000.0,              // 10 seconds max
            min_acceptance_rate: 0.9,                   // 90% minimum acceptance
            max_memory_usage_bytes: 1024 * 1024 * 1024, // 1 GB max
            max_cpu_utilization: 95.0,                  // 95% max CPU
            enabled_alerts,
        }
    }
}

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    /// Informational alerts for general status updates
    Info,
    /// Warning alerts for potentially concerning conditions
    Warning,
    /// Critical alerts for serious issues requiring attention
    Critical,
    /// Emergency alerts for immediate action required
    Emergency,
}

/// Alert message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// Alert severity
    pub severity: AlertSeverity,
    /// Alert category
    pub category: String,
    /// Human-readable message
    pub message: String,
    /// Additional context data
    pub context: HashMap<String, String>,
    /// Timestamp when alert was created (seconds since UNIX epoch)
    pub timestamp: u64,
}

/// Time-series data for tracking trends
///
/// This struct provides time-series analysis capabilities for monitoring metrics.
/// While currently used primarily for internal tracking, the analysis methods
/// (max, min, trend) are available for external monitoring integrations.
#[derive(Debug)]
pub struct TimeSeries {
    values: VecDeque<(Instant, f64)>,
    max_age: Duration,
    max_samples: usize,
}

impl TimeSeries {
    /// Create a new time series with specified maximum age and sample count
    pub fn new(max_age: Duration, max_samples: usize) -> Self {
        Self {
            values: VecDeque::new(),
            max_age,
            max_samples,
        }
    }

    /// Add a new sample to the time series
    pub fn add_sample(&mut self, value: f64) {
        let now = Instant::now();
        self.values.push_back((now, value));

        // Remove old samples
        while let Some(&(timestamp, _)) = self.values.front() {
            if now.duration_since(timestamp) > self.max_age {
                self.values.pop_front();
            } else {
                break;
            }
        }

        // Limit number of samples
        while self.values.len() > self.max_samples {
            self.values.pop_front();
        }
    }

    /// Calculate the average value of all samples in the time series
    pub fn average(&self) -> f64 {
        if self.values.is_empty() {
            0.0
        } else {
            let sum: f64 = self.values.iter().map(|(_, v)| v).sum();
            sum / self.values.len() as f64
        }
    }

    /// Get the maximum value in the time series
    pub fn max(&self) -> f64 {
        self.values.iter().map(|(_, v)| *v).fold(0.0, f64::max)
    }

    /// Get the minimum value in the time series
    pub fn min(&self) -> f64 {
        self.values
            .iter()
            .map(|(_, v)| *v)
            .fold(f64::INFINITY, f64::min)
    }

    /// Calculate the linear trend of the time series
    /// Returns the slope of the best-fit line through the data points
    pub fn trend(&self) -> f64 {
        if self.values.len() < 2 {
            return 0.0;
        }

        // Simple linear trend calculation
        let n = self.values.len() as f64;
        let x_sum: f64 = (0..self.values.len()).map(|i| i as f64).sum();
        let y_sum: f64 = self.values.iter().map(|(_, v)| v).sum();
        let xy_sum: f64 = self
            .values
            .iter()
            .enumerate()
            .map(|(i, (_, v))| i as f64 * v)
            .sum();
        let x2_sum: f64 = (0..self.values.len()).map(|i| (i as f64).powi(2)).sum();

        let denominator = n * x2_sum - x_sum.powi(2);
        if denominator.abs() < f64::EPSILON {
            0.0
        } else {
            (n * xy_sum - x_sum * y_sum) / denominator
        }
    }
}

/// Comprehensive monitoring system
pub struct MonitoringSystem {
    /// System configuration
    config: RwLock<AlertConfig>,
    /// Current performance metrics
    metrics: RwLock<PerformanceMetrics>,
    /// Recent alerts
    recent_alerts: RwLock<VecDeque<Alert>>,
    /// Time series data
    hash_rate_series: RwLock<TimeSeries>,
    response_time_series: RwLock<TimeSeries>,
    memory_usage_series: RwLock<TimeSeries>,
    /// Counters
    solutions_counter: AtomicU64,
    shares_counter: AtomicU64,
    accepted_shares_counter: AtomicU64,
    /// Status tracking
    system_start_time: Instant,
    monitoring_enabled: AtomicBool,
    last_health_check: RwLock<Instant>,
    /// HTTP pool reference for monitoring
    http_pool: Option<Arc<HttpClientPool>>,
}

impl MonitoringSystem {
    /// Create a new monitoring system
    pub fn new() -> Self {
        Self {
            config: RwLock::new(AlertConfig::default()),
            metrics: RwLock::new(PerformanceMetrics::default()),
            recent_alerts: RwLock::new(VecDeque::new()),
            hash_rate_series: RwLock::new(TimeSeries::new(Duration::from_secs(3600), 3600)),
            response_time_series: RwLock::new(TimeSeries::new(Duration::from_secs(3600), 3600)),
            memory_usage_series: RwLock::new(TimeSeries::new(Duration::from_secs(3600), 3600)),
            solutions_counter: AtomicU64::new(0),
            shares_counter: AtomicU64::new(0),
            accepted_shares_counter: AtomicU64::new(0),
            system_start_time: Instant::now(),
            monitoring_enabled: AtomicBool::new(true),
            last_health_check: RwLock::new(Instant::now()),
            http_pool: None,
        }
    }

    /// Create monitoring system with HTTP pool reference
    pub fn with_http_pool(http_pool: Arc<HttpClientPool>) -> Self {
        let mut monitor = Self::new();
        monitor.http_pool = Some(http_pool);
        monitor
    }

    /// Update configuration
    pub fn update_config(&self, config: AlertConfig) {
        *self.config.write() = config;
        info!("Monitoring configuration updated");
    }

    /// Record hash rate measurement
    pub fn record_hash_rate(&self, hash_rate: f64) {
        if !self.monitoring_enabled.load(Ordering::Relaxed) {
            return;
        }

        self.hash_rate_series.write().add_sample(hash_rate);

        let mut metrics = self.metrics.write();
        metrics.hash_rate = hash_rate;
        metrics.avg_hash_rate = self.hash_rate_series.read().average();
        metrics.peak_hash_rate = metrics.peak_hash_rate.max(hash_rate);

        // Check for alerts
        let config = self.config.read();
        if *config.enabled_alerts.get("hash_rate").unwrap_or(&true)
            && hash_rate < config.min_hash_rate
        {
            self.create_alert(
                AlertSeverity::Warning,
                "hash_rate",
                &format!(
                    "Hash rate {} H/s below minimum {}",
                    hash_rate, config.min_hash_rate
                ),
                vec![
                    ("current_rate".to_string(), hash_rate.to_string()),
                    ("minimum_rate".to_string(), config.min_hash_rate.to_string()),
                ],
            );
        }
    }

    /// Record response time measurement
    pub fn record_response_time(&self, response_time_ms: f64) {
        if !self.monitoring_enabled.load(Ordering::Relaxed) {
            return;
        }

        self.response_time_series
            .write()
            .add_sample(response_time_ms);

        let mut metrics = self.metrics.write();
        metrics.avg_response_time_ms = self.response_time_series.read().average();

        // Check for alerts
        let config = self.config.read();
        if *config.enabled_alerts.get("response_time").unwrap_or(&true)
            && response_time_ms > config.max_response_time_ms
        {
            self.create_alert(
                AlertSeverity::Critical,
                "response_time",
                &format!(
                    "Response time {:.2}ms exceeds maximum {:.2}ms",
                    response_time_ms, config.max_response_time_ms
                ),
                vec![
                    ("current_time".to_string(), response_time_ms.to_string()),
                    (
                        "maximum_time".to_string(),
                        config.max_response_time_ms.to_string(),
                    ),
                ],
            );
        }
    }

    /// Record solution found
    pub fn record_solution(&self) {
        self.solutions_counter.fetch_add(1, Ordering::Relaxed);
        let mut metrics = self.metrics.write();
        metrics.solutions_found = self.solutions_counter.load(Ordering::Relaxed);

        info!("Solution found - total: {}", metrics.solutions_found);
    }

    /// Record share submission
    pub fn record_share_submitted(&self, accepted: bool) {
        self.shares_counter.fetch_add(1, Ordering::Relaxed);
        if accepted {
            self.accepted_shares_counter.fetch_add(1, Ordering::Relaxed);
        }

        let total_shares = self.shares_counter.load(Ordering::Relaxed);
        let accepted_shares = self.accepted_shares_counter.load(Ordering::Relaxed);

        let mut metrics = self.metrics.write();
        metrics.shares_submitted = total_shares;
        metrics.acceptance_rate = if total_shares > 0 {
            accepted_shares as f64 / total_shares as f64
        } else {
            0.0
        };

        // Check acceptance rate alerts
        let config = self.config.read();
        if *config
            .enabled_alerts
            .get("acceptance_rate")
            .unwrap_or(&true)
            && total_shares >= 10
            && metrics.acceptance_rate < config.min_acceptance_rate
        {
            self.create_alert(
                AlertSeverity::Warning,
                "acceptance_rate",
                &format!(
                    "Share acceptance rate {:.2}% below minimum {:.2}%",
                    metrics.acceptance_rate * 100.0,
                    config.min_acceptance_rate * 100.0
                ),
                vec![
                    (
                        "current_rate".to_string(),
                        format!("{:.2}", metrics.acceptance_rate),
                    ),
                    (
                        "minimum_rate".to_string(),
                        format!("{:.2}", config.min_acceptance_rate),
                    ),
                    ("total_shares".to_string(), total_shares.to_string()),
                    ("accepted_shares".to_string(), accepted_shares.to_string()),
                ],
            );
        }
    }

    /// Update memory usage
    pub fn record_memory_usage(&self, memory_bytes: u64) {
        if !self.monitoring_enabled.load(Ordering::Relaxed) {
            return;
        }

        self.memory_usage_series
            .write()
            .add_sample(memory_bytes as f64);

        let mut metrics = self.metrics.write();
        metrics.memory_usage_bytes = memory_bytes;

        // Check memory usage alerts
        let config = self.config.read();
        if *config.enabled_alerts.get("memory_usage").unwrap_or(&true)
            && memory_bytes > config.max_memory_usage_bytes
        {
            self.create_alert(
                AlertSeverity::Critical,
                "memory_usage",
                &format!(
                    "Memory usage {} bytes exceeds maximum {}",
                    memory_bytes, config.max_memory_usage_bytes
                ),
                vec![
                    ("current_usage".to_string(), memory_bytes.to_string()),
                    (
                        "maximum_usage".to_string(),
                        config.max_memory_usage_bytes.to_string(),
                    ),
                ],
            );
        }
    }

    /// Update CPU utilization
    pub fn record_cpu_utilization(&self, cpu_percent: f64) {
        if !self.monitoring_enabled.load(Ordering::Relaxed) {
            return;
        }

        let mut metrics = self.metrics.write();
        metrics.cpu_utilization = cpu_percent;

        // Check CPU usage alerts
        let config = self.config.read();
        if *config.enabled_alerts.get("cpu_usage").unwrap_or(&true)
            && cpu_percent > config.max_cpu_utilization
        {
            self.create_alert(
                AlertSeverity::Warning,
                "cpu_usage",
                &format!(
                    "CPU utilization {:.1}% exceeds maximum {:.1}%",
                    cpu_percent, config.max_cpu_utilization
                ),
                vec![
                    ("current_usage".to_string(), cpu_percent.to_string()),
                    (
                        "maximum_usage".to_string(),
                        config.max_cpu_utilization.to_string(),
                    ),
                ],
            );
        }
    }

    /// Perform comprehensive health check
    pub fn health_check(&self) -> HealthStatus {
        let now = Instant::now();
        *self.last_health_check.write() = now;

        let metrics = self.metrics.read();
        let config = self.config.read();

        // Check various health indicators
        let mut issues = Vec::new();

        // Hash rate check
        if metrics.hash_rate < config.min_hash_rate {
            issues.push("Low hash rate");
        }

        // Response time check
        if metrics.avg_response_time_ms > config.max_response_time_ms {
            issues.push("High response time");
        }

        // Acceptance rate check
        if metrics.shares_submitted > 10 && metrics.acceptance_rate < config.min_acceptance_rate {
            issues.push("Low acceptance rate");
        }

        // Memory usage check
        if metrics.memory_usage_bytes > config.max_memory_usage_bytes {
            issues.push("High memory usage");
        }

        // CPU usage check
        if metrics.cpu_utilization > config.max_cpu_utilization {
            issues.push("High CPU usage");
        }

        // HTTP pool health check
        if let Some(ref pool) = self.http_pool {
            let pool_stats = pool.get_stats();
            if let Some(hit_rate) = pool_stats.cache_hit_rate {
                if hit_rate < 0.8 {
                    issues.push("Low HTTP cache hit rate");
                }
            }
        }

        // Determine overall health status
        let status = match issues.len() {
            0 => HealthStatus::Healthy,
            1..=2 => HealthStatus::Warning,
            3..=4 => HealthStatus::Critical,
            _ => HealthStatus::Down,
        };

        if !issues.is_empty() {
            debug!("Health check found issues: {:?}", issues);
        }

        status
    }

    /// Get current performance metrics
    pub fn get_metrics(&self) -> PerformanceMetrics {
        let mut metrics = self.metrics.read().clone();
        metrics.uptime_seconds = self.system_start_time.elapsed().as_secs();
        metrics
    }

    /// Get recent alerts
    pub fn get_recent_alerts(&self, max_count: usize) -> Vec<Alert> {
        let alerts = self.recent_alerts.read();
        alerts.iter().take(max_count).cloned().collect()
    }

    /// Clear old alerts
    pub fn clear_old_alerts(&self, max_age: Duration) {
        let mut alerts = self.recent_alerts.write();
        let cutoff_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(max_age.as_secs());

        while let Some(alert) = alerts.front() {
            if alert.timestamp < cutoff_timestamp {
                alerts.pop_front();
            } else {
                break;
            }
        }
    }

    /// Enable or disable monitoring
    pub fn set_monitoring_enabled(&self, enabled: bool) {
        self.monitoring_enabled.store(enabled, Ordering::Relaxed);
        if enabled {
            info!("Monitoring enabled");
        } else {
            warn!("Monitoring disabled");
        }
    }

    /// Generate comprehensive status report
    pub fn generate_status_report(&self) -> String {
        let metrics = self.get_metrics();
        let health = self.health_check();
        let recent_alerts = self.get_recent_alerts(5);

        let mut report = String::new();
        report.push_str("=== Mining Client Status Report ===\n");
        report.push_str(&format!("Health Status: {:?}\n", health));
        report.push_str(&format!("Uptime: {} seconds\n", metrics.uptime_seconds));
        report.push_str("\n--- Performance Metrics ---\n");
        report.push_str(&format!(
            "Hash Rate: {:.2} H/s (avg: {:.2}, peak: {:.2})\n",
            metrics.hash_rate, metrics.avg_hash_rate, metrics.peak_hash_rate
        ));
        report.push_str(&format!("Solutions Found: {}\n", metrics.solutions_found));
        report.push_str(&format!(
            "Shares Submitted: {} (acceptance: {:.1}%)\n",
            metrics.shares_submitted,
            metrics.acceptance_rate * 100.0
        ));
        report.push_str(&format!(
            "Response Time: {:.2} ms\n",
            metrics.avg_response_time_ms
        ));
        report.push_str(&format!(
            "Memory Usage: {} bytes\n",
            metrics.memory_usage_bytes
        ));
        report.push_str(&format!(
            "CPU Utilization: {:.1}%\n",
            metrics.cpu_utilization
        ));

        if !recent_alerts.is_empty() {
            report.push_str(&format!(
                "\n--- Recent Alerts ({}) ---\n",
                recent_alerts.len()
            ));
            for alert in recent_alerts {
                report.push_str(&format!(
                    "[{:?}] {}: {}\n",
                    alert.severity, alert.category, alert.message
                ));
            }
        }

        if let Some(ref pool) = self.http_pool {
            let pool_stats = pool.get_stats();
            report.push_str("\n--- HTTP Pool Stats ---\n");
            report.push_str(&format!("Active Clients: {}\n", pool_stats.active_clients));
            report.push_str(&format!("Client Types: {:?}\n", pool_stats.client_types));
            if let Some(hit_rate) = pool_stats.cache_hit_rate {
                report.push_str(&format!("Cache Hit Rate: {:.1}%\n", hit_rate * 100.0));
            }
        }

        report
    }

    /// Internal method to create alerts
    fn create_alert(
        &self,
        severity: AlertSeverity,
        category: &str,
        message: &str,
        context: Vec<(String, String)>,
    ) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let alert = Alert {
            severity,
            category: category.to_string(),
            message: message.to_string(),
            context: context.into_iter().collect(),
            timestamp,
        };

        // Log the alert
        match severity {
            AlertSeverity::Info => info!("[ALERT] {}: {}", category, message),
            AlertSeverity::Warning => warn!("[ALERT] {}: {}", category, message),
            AlertSeverity::Critical => error!("[ALERT] {}: {}", category, message),
            AlertSeverity::Emergency => error!("[EMERGENCY] {}: {}", category, message),
        }

        // Store the alert
        let mut alerts = self.recent_alerts.write();
        alerts.push_back(alert);

        // Limit number of stored alerts
        while alerts.len() > 1000 {
            alerts.pop_front();
        }
    }
}

impl Default for MonitoringSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Global monitoring instance
static MONITORING: std::sync::OnceLock<MonitoringSystem> = std::sync::OnceLock::new();

/// Get the global monitoring system instance
pub fn global_monitoring() -> &'static MonitoringSystem {
    MONITORING.get_or_init(MonitoringSystem::new)
}

/// Initialize global monitoring with HTTP pool
pub fn init_monitoring_with_pool(http_pool: Arc<HttpClientPool>) {
    let _ = MONITORING.set(MonitoringSystem::with_http_pool(http_pool));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_monitoring_system_creation() {
        let monitor = MonitoringSystem::new();
        let metrics = monitor.get_metrics();
        assert_eq!(metrics.hash_rate, 0.0);
        assert_eq!(metrics.solutions_found, 0);
    }

    #[test]
    fn test_hash_rate_recording() {
        let monitor = MonitoringSystem::new();
        monitor.record_hash_rate(1500.0);

        let metrics = monitor.get_metrics();
        assert_eq!(metrics.hash_rate, 1500.0);
        assert_eq!(metrics.peak_hash_rate, 1500.0);
    }

    #[test]
    fn test_solution_recording() {
        let monitor = MonitoringSystem::new();
        monitor.record_solution();
        monitor.record_solution();

        let metrics = monitor.get_metrics();
        assert_eq!(metrics.solutions_found, 2);
    }

    #[test]
    fn test_share_submission_recording() {
        let monitor = MonitoringSystem::new();
        monitor.record_share_submitted(true);
        monitor.record_share_submitted(true);
        monitor.record_share_submitted(false);

        let metrics = monitor.get_metrics();
        assert_eq!(metrics.shares_submitted, 3);
        assert!((metrics.acceptance_rate - (2.0 / 3.0)).abs() < 0.001);
    }

    #[test]
    fn test_health_check() {
        let monitor = MonitoringSystem::new();
        let health = monitor.health_check();
        // With default values and no data, should be healthy
        assert_eq!(health, HealthStatus::Healthy);
    }

    #[test]
    fn test_alert_generation() {
        let monitor = MonitoringSystem::new();

        // Trigger a low hash rate alert
        monitor.record_hash_rate(500.0); // Below default minimum of 1000

        let alerts = monitor.get_recent_alerts(10);
        assert!(!alerts.is_empty());
        assert_eq!(alerts[0].category, "hash_rate");
        assert_eq!(alerts[0].severity, AlertSeverity::Warning);
    }

    #[test]
    fn test_time_series() {
        let mut series = TimeSeries::new(Duration::from_secs(60), 100);

        series.add_sample(10.0);
        series.add_sample(20.0);
        series.add_sample(30.0);

        assert_eq!(series.average(), 20.0);
        assert_eq!(series.max(), 30.0);
        assert_eq!(series.min(), 10.0);
    }

    #[test]
    fn test_monitoring_enable_disable() {
        let monitor = MonitoringSystem::new();

        monitor.set_monitoring_enabled(false);
        monitor.record_hash_rate(1000.0); // Should be ignored

        let metrics = monitor.get_metrics();
        assert_eq!(metrics.hash_rate, 0.0); // Should remain 0

        monitor.set_monitoring_enabled(true);
        monitor.record_hash_rate(1000.0); // Should be recorded

        let metrics = monitor.get_metrics();
        assert_eq!(metrics.hash_rate, 1000.0);
    }

    #[test]
    fn test_status_report_generation() {
        let monitor = MonitoringSystem::new();
        monitor.record_hash_rate(1500.0);
        monitor.record_solution();

        let report = monitor.generate_status_report();
        assert!(report.contains("Hash Rate: 1500.00"));
        assert!(report.contains("Solutions Found: 1"));
        assert!(report.contains("Health Status"));
    }
}
