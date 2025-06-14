//! Work preemption logic for efficient mining updates
//!
//! This module provides smart work preemption strategies that minimize
//! downtime when new work becomes available, improving mining efficiency.

use crate::core::{Target, Work};
use crate::error::Result;
use crate::workers::{MiningResult, Worker};
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Strategy for handling work preemption
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreemptionStrategy {
    /// Immediately stop and switch to new work
    Immediate,
    /// Allow current batch to complete, then switch
    BatchComplete,
    /// Wait for a brief period before switching (allows for potential rapid updates)
    Delayed(Duration),
    /// Only preempt if the new work is significantly different
    Conditional,
}

impl Default for PreemptionStrategy {
    fn default() -> Self {
        PreemptionStrategy::Immediate
    }
}

/// Configuration for work preemption behavior
#[derive(Debug, Clone)]
pub struct PreemptionConfig {
    /// Strategy to use for preemption
    pub strategy: PreemptionStrategy,
    /// Minimum time between preemptions to avoid thrashing
    pub min_preemption_interval: Duration,
    /// Maximum time to wait for new work before giving up
    pub max_work_fetch_time: Duration,
    /// Whether to validate that new work is actually different
    pub validate_work_change: bool,
}

impl Default for PreemptionConfig {
    fn default() -> Self {
        Self {
            strategy: PreemptionStrategy::Immediate,
            min_preemption_interval: Duration::from_millis(100),
            max_work_fetch_time: Duration::from_secs(5),
            validate_work_change: true,
        }
    }
}

/// Statistics about work preemption events
#[derive(Debug, Clone, Default)]
pub struct PreemptionStats {
    /// Total number of preemption events
    pub total_preemptions: u64,
    /// Number of preemptions that were skipped due to rate limiting
    pub skipped_preemptions: u64,
    /// Number of preemptions where new work was identical
    pub identical_work_skips: u64,
    /// Average time to fetch new work
    pub avg_work_fetch_time_ms: f64,
    /// Average time to restart mining after preemption
    pub avg_restart_time_ms: f64,
    /// Total downtime due to preemptions
    pub total_downtime_ms: u64,
}

impl PreemptionStats {
    /// Update work fetch timing statistics
    pub fn record_work_fetch(&mut self, duration: Duration) {
        let duration_ms = duration.as_millis() as f64;
        self.avg_work_fetch_time_ms = (self.avg_work_fetch_time_ms * self.total_preemptions as f64
            + duration_ms)
            / (self.total_preemptions + 1) as f64;
    }

    /// Update restart timing statistics  
    pub fn record_restart(&mut self, duration: Duration) {
        let duration_ms = duration.as_millis() as f64;
        self.avg_restart_time_ms = (self.avg_restart_time_ms * self.total_preemptions as f64
            + duration_ms)
            / (self.total_preemptions + 1) as f64;
    }

    /// Record total downtime
    pub fn record_downtime(&mut self, duration: Duration) {
        self.total_downtime_ms += duration.as_millis() as u64;
    }

    /// Calculate preemption efficiency (percentage of time spent mining vs. preempting)
    pub fn efficiency_percentage(&self, total_mining_time: Duration) -> f64 {
        let total_time_ms = total_mining_time.as_millis() as f64;
        if total_time_ms == 0.0 {
            return 100.0;
        }
        ((total_time_ms - self.total_downtime_ms as f64) / total_time_ms) * 100.0
    }
}

/// Work preemption coordinator
#[derive(Debug)]
pub struct WorkPreemptor {
    config: PreemptionConfig,
    stats: parking_lot::Mutex<PreemptionStats>,
    last_preemption: parking_lot::Mutex<Option<Instant>>,
}

impl WorkPreemptor {
    /// Create a new work preemptor with the given configuration
    pub fn new(config: PreemptionConfig) -> Self {
        Self {
            config,
            stats: parking_lot::Mutex::new(PreemptionStats::default()),
            last_preemption: parking_lot::Mutex::new(None),
        }
    }

    /// Create a work preemptor with default configuration
    pub fn with_defaults() -> Self {
        Self::new(PreemptionConfig::default())
    }

    /// Determine if preemption should occur based on configuration and timing
    pub fn should_preempt(&self, new_work: &Work, current_work: &Work) -> PreemptionDecision {
        let now = Instant::now();

        // Check rate limiting
        if let Some(last_preemption) = *self.last_preemption.lock() {
            if now.duration_since(last_preemption) < self.config.min_preemption_interval {
                self.stats.lock().skipped_preemptions += 1;
                return PreemptionDecision::Skip(PreemptionSkipReason::RateLimited);
            }
        }

        // Validate work change if configured
        if self.config.validate_work_change && self.is_work_identical(new_work, current_work) {
            self.stats.lock().identical_work_skips += 1;
            return PreemptionDecision::Skip(PreemptionSkipReason::IdenticalWork);
        }

        // Apply strategy-specific logic
        match self.config.strategy {
            PreemptionStrategy::Immediate => {
                PreemptionDecision::Preempt(PreemptionAction::Immediate)
            }
            PreemptionStrategy::BatchComplete => {
                PreemptionDecision::Preempt(PreemptionAction::AfterBatch)
            }
            PreemptionStrategy::Delayed(duration) => {
                PreemptionDecision::Preempt(PreemptionAction::Delayed(duration))
            }
            PreemptionStrategy::Conditional => {
                // More sophisticated logic could be added here
                if self.is_work_significantly_different(new_work, current_work) {
                    PreemptionDecision::Preempt(PreemptionAction::Immediate)
                } else {
                    PreemptionDecision::Skip(PreemptionSkipReason::MinorChange)
                }
            }
        }
    }

    /// Execute preemption with the specified action
    pub async fn execute_preemption<F, Fut>(
        &self,
        action: PreemptionAction,
        worker: Arc<dyn Worker>,
        new_work: Work,
        new_target: Target,
        result_tx: mpsc::Sender<MiningResult>,
        _work_fetch_fn: F,
    ) -> Result<()>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(Work, Target)>>,
    {
        let preemption_start = Instant::now();

        debug!("Executing preemption with action: {:?}", action);

        match action {
            PreemptionAction::Immediate => {
                self.immediate_preemption(worker, new_work, new_target, result_tx)
                    .await?;
            }
            PreemptionAction::AfterBatch => {
                // For batch completion, we could implement a more sophisticated approach
                // For now, fall back to immediate preemption
                warn!("Batch completion preemption not fully implemented, using immediate");
                self.immediate_preemption(worker, new_work, new_target, result_tx)
                    .await?;
            }
            PreemptionAction::Delayed(delay) => {
                tokio::time::sleep(delay).await;
                self.immediate_preemption(worker, new_work, new_target, result_tx)
                    .await?;
            }
        }

        // Update statistics
        let total_time = preemption_start.elapsed();
        let mut stats = self.stats.lock();
        stats.total_preemptions += 1;
        stats.record_downtime(total_time);
        *self.last_preemption.lock() = Some(preemption_start);

        info!("Work preemption completed in {:?}", total_time);
        Ok(())
    }

    /// Execute immediate preemption
    async fn immediate_preemption(
        &self,
        worker: Arc<dyn Worker>,
        new_work: Work,
        new_target: Target,
        result_tx: mpsc::Sender<MiningResult>,
    ) -> Result<()> {
        let _stop_start = Instant::now();

        // Stop current mining
        worker.stop().await?;

        let restart_start = Instant::now();

        // Start mining with new work
        worker.mine(new_work, new_target, result_tx).await?;

        // Update timing statistics
        let mut stats = self.stats.lock();
        stats.record_restart(restart_start.elapsed());

        Ok(())
    }

    /// Check if two work items are functionally identical
    fn is_work_identical(&self, work1: &Work, work2: &Work) -> bool {
        // Compare the work bytes (excluding nonce which might be different)
        let bytes1 = work1.as_bytes();
        let bytes2 = work2.as_bytes();

        // Compare everything except the nonce field (typically at the end)
        if bytes1.len() >= 8 && bytes2.len() >= 8 {
            // Compare all bytes except the last 8 (nonce)
            bytes1[..bytes1.len() - 8] == bytes2[..bytes2.len() - 8]
        } else {
            bytes1 == bytes2
        }
    }

    /// Check if two work items are significantly different (for conditional preemption)
    fn is_work_significantly_different(&self, work1: &Work, work2: &Work) -> bool {
        // For now, any difference is considered significant
        // This could be enhanced with more sophisticated logic
        !self.is_work_identical(work1, work2)
    }

    /// Get current preemption statistics
    pub fn get_stats(&self) -> PreemptionStats {
        self.stats.lock().clone()
    }

    /// Reset preemption statistics
    pub fn reset_stats(&self) {
        *self.stats.lock() = PreemptionStats::default();
        *self.last_preemption.lock() = None;
    }

    /// Update configuration
    pub fn update_config(&mut self, config: PreemptionConfig) {
        self.config = config;
        info!("Preemption configuration updated: {:?}", self.config);
    }
}

/// Decision about whether to preempt work
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreemptionDecision {
    /// Preempt with the specified action
    Preempt(PreemptionAction),
    /// Skip preemption for the specified reason
    Skip(PreemptionSkipReason),
}

/// Action to take when preempting work
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreemptionAction {
    /// Stop immediately and start new work
    Immediate,
    /// Complete current batch then start new work
    AfterBatch,
    /// Wait for specified duration then start new work
    Delayed(Duration),
}

/// Reason for skipping preemption
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreemptionSkipReason {
    /// Preemption rate limited (too soon since last preemption)
    RateLimited,
    /// New work is identical to current work
    IdenticalWork,
    /// New work is only a minor change
    MinorChange,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::constants::WORK_SIZE;

    #[test]
    fn test_preemption_config_default() {
        let config = PreemptionConfig::default();
        assert_eq!(config.strategy, PreemptionStrategy::Immediate);
        assert_eq!(config.min_preemption_interval, Duration::from_millis(100));
        assert!(config.validate_work_change);
    }

    #[test]
    fn test_preemption_stats() {
        let mut stats = PreemptionStats::default();

        stats.record_work_fetch(Duration::from_millis(50));
        assert_eq!(stats.avg_work_fetch_time_ms, 50.0);

        stats.record_restart(Duration::from_millis(25));
        assert_eq!(stats.avg_restart_time_ms, 25.0);

        stats.record_downtime(Duration::from_millis(100));
        assert_eq!(stats.total_downtime_ms, 100);

        let efficiency = stats.efficiency_percentage(Duration::from_secs(1));
        assert!(efficiency > 80.0); // Should be around 90%
    }

    #[test]
    fn test_work_preemptor_creation() {
        let config = PreemptionConfig::default();
        let preemptor = WorkPreemptor::new(config);

        let stats = preemptor.get_stats();
        assert_eq!(stats.total_preemptions, 0);
        assert_eq!(stats.skipped_preemptions, 0);
    }

    #[test]
    fn test_preemption_decision_immediate() {
        let preemptor = WorkPreemptor::with_defaults();

        let work1 = Work::from_bytes([1u8; WORK_SIZE]);
        let work2 = Work::from_bytes([2u8; WORK_SIZE]);

        let decision = preemptor.should_preempt(&work2, &work1);
        assert_eq!(
            decision,
            PreemptionDecision::Preempt(PreemptionAction::Immediate)
        );
    }

    #[test]
    fn test_preemption_decision_identical_work() {
        let preemptor = WorkPreemptor::with_defaults();

        let work1 = Work::from_bytes([1u8; WORK_SIZE]);
        let work2 = Work::from_bytes([1u8; WORK_SIZE]);

        let decision = preemptor.should_preempt(&work2, &work1);
        assert_eq!(
            decision,
            PreemptionDecision::Skip(PreemptionSkipReason::IdenticalWork)
        );
    }

    #[test]
    fn test_preemption_rate_limiting() {
        let mut config = PreemptionConfig::default();
        config.min_preemption_interval = Duration::from_millis(1000);
        let preemptor = WorkPreemptor::new(config);

        let work1 = Work::from_bytes([1u8; WORK_SIZE]);
        let work2 = Work::from_bytes([2u8; WORK_SIZE]);
        let work3 = Work::from_bytes([3u8; WORK_SIZE]);

        // First preemption should be allowed
        let decision1 = preemptor.should_preempt(&work2, &work1);
        assert_eq!(
            decision1,
            PreemptionDecision::Preempt(PreemptionAction::Immediate)
        );

        // Simulate preemption
        *preemptor.last_preemption.lock() = Some(Instant::now());

        // Second preemption should be rate limited
        let decision2 = preemptor.should_preempt(&work3, &work2);
        assert_eq!(
            decision2,
            PreemptionDecision::Skip(PreemptionSkipReason::RateLimited)
        );
    }

    #[test]
    fn test_work_identical_comparison() {
        let preemptor = WorkPreemptor::with_defaults();

        let work1 = Work::from_bytes([1u8; WORK_SIZE]);
        let work2 = Work::from_bytes([1u8; WORK_SIZE]);
        let work3 = Work::from_bytes([2u8; WORK_SIZE]);

        assert!(preemptor.is_work_identical(&work1, &work2));
        assert!(!preemptor.is_work_identical(&work1, &work3));
    }

    #[test]
    fn test_preemption_strategy_delayed() {
        let mut config = PreemptionConfig::default();
        config.strategy = PreemptionStrategy::Delayed(Duration::from_millis(100));
        let preemptor = WorkPreemptor::new(config);

        let work1 = Work::from_bytes([1u8; WORK_SIZE]);
        let work2 = Work::from_bytes([2u8; WORK_SIZE]);

        let decision = preemptor.should_preempt(&work2, &work1);
        assert_eq!(
            decision,
            PreemptionDecision::Preempt(PreemptionAction::Delayed(Duration::from_millis(100)))
        );
    }

    #[test]
    fn test_stats_reset() {
        let preemptor = WorkPreemptor::with_defaults();

        // Simulate some activity
        {
            let mut stats = preemptor.stats.lock();
            stats.total_preemptions = 5;
            stats.skipped_preemptions = 2;
        }

        preemptor.reset_stats();

        let stats = preemptor.get_stats();
        assert_eq!(stats.total_preemptions, 0);
        assert_eq!(stats.skipped_preemptions, 0);
    }
}
