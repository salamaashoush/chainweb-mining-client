//! Constant delay mining worker for non-PoW testing

use crate::core::{Nonce, Target, Work};
use crate::error::Result;
use crate::workers::{MiningResult, Worker};
use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, info};

/// Configuration for constant delay mining
#[derive(Debug, Clone)]
pub struct ConstantDelayWorkerConfig {
    /// Block time in seconds
    pub block_time_secs: u64,
}

/// Constant delay mining worker
///
/// This worker produces blocks at a constant rate, ignoring difficulty.
/// Useful for testing in development mode with DISABLE_POW_VALIDATION=1.
pub struct ConstantDelayWorker {
    config: ConstantDelayWorkerConfig,
    running: Arc<AtomicBool>,
}

impl ConstantDelayWorker {
    /// Create a new constant delay worker
    pub fn new(config: ConstantDelayWorkerConfig) -> Self {
        info!(
            "Initializing constant delay worker with {} second block time",
            config.block_time_secs
        );

        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl Worker for ConstantDelayWorker {
    async fn mine(
        &self,
        work: Work,
        _target: Target, // Ignored in constant delay mode
        result_tx: mpsc::Sender<MiningResult>,
    ) -> Result<()> {
        self.running.store(true, Ordering::Relaxed);

        let running = Arc::clone(&self.running);
        let block_time = Duration::from_secs(self.config.block_time_secs);
        let work = work.clone();

        tokio::spawn(async move {
            info!("Starting constant delay mining");

            let mut ticker = interval(block_time);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

            loop {
                ticker.tick().await;

                if !running.load(Ordering::Relaxed) {
                    debug!("Constant delay mining stopped");
                    break;
                }

                // Generate a sequential nonce for debugging
                static NONCE_COUNTER: AtomicU64 = AtomicU64::new(0);
                let nonce_value = NONCE_COUNTER.fetch_add(1, Ordering::Relaxed);
                let nonce = Nonce::new(nonce_value);

                // In constant delay mode, we don't compute the hash
                // We just return the work with the nonce unchanged
                let result = MiningResult {
                    work: work.clone(),
                    nonce,
                    hash: [0u8; 32], // Fake hash
                };

                info!("Constant delay block produced with nonce: {}", nonce);

                if let Err(e) = result_tx.send(result).await {
                    debug!("Failed to send mining result: {}", e);
                    break;
                }
            }
        });

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn worker_type(&self) -> &str {
        "ConstantDelay"
    }

    async fn hashrate(&self) -> u64 {
        // Constant delay doesn't have a meaningful hashrate
        // Return blocks per hour as a rough metric
        if self.running.load(Ordering::Relaxed) {
            3600 / self.config.block_time_secs
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Target;
    use std::time::Instant;

    #[tokio::test]
    #[ignore] // Timing-sensitive test, may be flaky in CI
    async fn test_constant_delay_worker() {
        let config = ConstantDelayWorkerConfig {
            block_time_secs: 1, // 1 second for fast test
        };

        let worker = ConstantDelayWorker::new(config);
        let (tx, mut rx) = mpsc::channel(10);

        let work = Work::from_bytes([0u8; 286]);
        let target = Target::from_bytes([0xFF; 32]);

        // Start mining
        let start = Instant::now();
        worker.mine(work, target, tx).await.unwrap();

        // Wait for first block (with some tolerance for timing)
        let result1 = rx.recv().await.unwrap();
        let elapsed1 = start.elapsed();
        assert!(elapsed1 >= Duration::from_millis(800)); // Allow some tolerance
        assert!(elapsed1 < Duration::from_secs(3)); // More lenient upper bound

        // Wait for second block
        let result2 = rx.recv().await.unwrap();
        let elapsed2 = start.elapsed();
        assert!(elapsed2 >= Duration::from_millis(1800)); // Allow some tolerance
        assert!(elapsed2 < Duration::from_secs(4)); // More lenient upper bound

        // Nonces should be different
        assert_ne!(result1.nonce.value(), result2.nonce.value());

        // Stop mining
        worker.stop().await.unwrap();
    }
}
