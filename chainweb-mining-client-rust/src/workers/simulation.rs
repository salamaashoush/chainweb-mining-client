//! Simulated mining worker for testing

use crate::core::{Nonce, Target, Work};
use crate::error::Result;
use crate::workers::{MiningResult, Worker};
use async_trait::async_trait;
use rand::rng;
use rand_distr::{Distribution, Exp};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{debug, info};

/// Configuration for simulated mining
#[derive(Debug, Clone)]
pub struct SimulationWorkerConfig {
    /// Target hash rate (hashes per second)
    pub hash_rate: f64,
}

/// Simulated mining worker
pub struct SimulationWorker {
    config: SimulationWorkerConfig,
    running: Arc<AtomicBool>,
    current_hashrate: Arc<AtomicU64>,
}

impl SimulationWorker {
    /// Create a new simulation worker
    pub fn new(config: SimulationWorkerConfig) -> Self {
        info!(
            "Initializing simulation worker with {} H/s",
            config.hash_rate
        );

        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            current_hashrate: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Calculate expected time to find a block
    fn calculate_block_time(&self, target: &Target) -> Duration {
        // Get the difficulty from the target
        let difficulty = target.to_difficulty();

        // Expected time = difficulty / hash_rate
        let expected_seconds = difficulty / self.config.hash_rate;

        // Use exponential distribution for realistic mining simulation
        let exp = Exp::new(1.0 / expected_seconds).unwrap();
        let actual_seconds = exp.sample(&mut rng());

        Duration::from_secs_f64(actual_seconds)
    }
}

#[async_trait]
impl Worker for SimulationWorker {
    async fn mine(
        &self,
        work: Work,
        target: Target,
        result_tx: mpsc::Sender<MiningResult>,
    ) -> Result<()> {
        self.running.store(true, Ordering::Relaxed);
        self.current_hashrate
            .store(self.config.hash_rate as u64, Ordering::Relaxed);

        let running = Arc::clone(&self.running);
        let work = work.clone();
        let block_time = self.calculate_block_time(&target);

        tokio::spawn(async move {
            info!("Starting simulated mining");
            debug!("Simulated block time: {:?}", block_time);

            // Create a timer
            let sleep_fut = sleep(block_time);
            tokio::pin!(sleep_fut);

            // Wait for either the timer or stop signal
            loop {
                tokio::select! {
                    _ = &mut sleep_fut => {
                        // Timer expired, we "found" a block
                        if running.load(Ordering::Relaxed) {
                            // Generate a random nonce
                            let nonce = Nonce::new(rand::random());

                            // In simulation mode, we don't actually compute the hash
                            // We just return a fake result
                            let result = MiningResult {
                                work: work.clone(),
                                nonce,
                                hash: [0u8; 32], // Fake hash
                            };

                            info!("Simulation found block with nonce: {}", nonce);

                            if let Err(e) = result_tx.send(result).await {
                                debug!("Failed to send mining result: {}", e);
                            }
                        }
                        break;
                    }
                    _ = sleep(Duration::from_millis(100)) => {
                        // Check if we should stop
                        if !running.load(Ordering::Relaxed) {
                            debug!("Simulation mining stopped");
                            break;
                        }
                    }
                }
            }
        });

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::Relaxed);
        self.current_hashrate.store(0, Ordering::Relaxed);
        Ok(())
    }

    fn worker_type(&self) -> &str {
        "Simulation"
    }

    async fn hashrate(&self) -> u64 {
        self.current_hashrate.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Target;

    #[tokio::test]
    async fn test_simulation_worker() {
        let config = SimulationWorkerConfig {
            hash_rate: 1_000_000.0, // 1 MH/s
        };

        let worker = SimulationWorker::new(config);
        let (tx, _rx) = mpsc::channel(10);

        // Easy target for fast test
        let work = Work::from_bytes([0u8; 286]);
        let target = Target::from_bytes([0xFF; 32]);

        // Start mining
        worker.mine(work, target, tx).await.unwrap();

        // Check hashrate
        let hashrate = worker.hashrate().await;
        assert_eq!(hashrate, 1_000_000);

        // Stop mining
        worker.stop().await.unwrap();

        // Hashrate should be 0 after stopping
        let hashrate = worker.hashrate().await;
        assert_eq!(hashrate, 0);
    }
}
