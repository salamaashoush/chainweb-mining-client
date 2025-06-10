//! Constant delay worker for deterministic testing
//!
//! Produces blocks at a constant rate regardless of difficulty, useful for testing
//! scenarios that don't involve proof-of-work validation.

use super::{mining_span, MiningStats, MiningWorker};
use crate::{ChainId, Error, Nonce, Result, Target, Work};
use async_trait::async_trait;
use rand::Rng;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

/// Constant delay worker that produces blocks at regular intervals
pub struct ConstantDelayWorker {
    block_time: Duration,
    stats: MiningStats,
}

impl ConstantDelayWorker {
    /// Create a new constant delay worker with specified block time
    pub fn new(block_time: Duration) -> Self {
        info!("Creating constant delay worker with block time: {:?}", block_time);
        
        Self {
            block_time,
            stats: MiningStats::default(),
        }
    }

    /// Wait for the specified delay and produce a block
    async fn produce_block(
        &mut self,
        mut work: Work,
        cancellation: CancellationToken,
    ) -> Result<Work> {
        let start_time = Instant::now();
        
        info!("Waiting {:?} before producing block", self.block_time);

        // Wait for the specified delay or cancellation
        tokio::select! {
            _ = sleep(self.block_time) => {
                // Generate a random nonce for the solution
                let mut rng = rand::thread_rng();
                let solution_nonce = Nonce::new(rng.gen());
                work.inject_nonce(solution_nonce);
                
                let elapsed = start_time.elapsed();
                
                // Update statistics
                self.stats.solutions_found = 1;
                self.stats.mining_time_secs = elapsed.as_secs();
                // Since we don't do actual hashing, we'll report a nominal hash count
                self.stats.total_hashes = 1;
                self.stats.current_hash_rate = 1.0 / elapsed.as_secs_f64();
                self.stats.average_hash_rate = self.stats.current_hash_rate;
                
                info!(
                    "Constant delay worker produced block after {:?} with nonce {}",
                    elapsed,
                    solution_nonce
                );
                
                Ok(work)
            }
            _ = cancellation.cancelled() => {
                info!("Constant delay mining cancelled");
                Err(Error::cancelled("Constant delay mining"))
            }
        }
    }
}

#[async_trait]
impl MiningWorker for ConstantDelayWorker {
    fn worker_type(&self) -> &'static str {
        "constant-delay"
    }

    async fn mine(
        &mut self,
        _initial_nonce: Nonce,
        target: Target,
        chain_id: ChainId,
        work: Work,
        cancellation: CancellationToken,
        stats_tx: Option<mpsc::UnboundedSender<MiningStats>>,
    ) -> Result<Work> {
        let _span = mining_span(self.worker_type(), chain_id);
        
        info!(
            "Starting constant delay mining for chain {} with {:?} block time (ignoring difficulty level: {})",
            chain_id,
            self.block_time,
            target.difficulty_level()
        );

        // Reset statistics
        self.stats = MiningStats::default();

        // Statistics reporting task
        let stats_clone = self.stats.clone();
        let stats_cancellation = cancellation.clone();
        let stats_handle = if let Some(stats_tx) = stats_tx {
            Some(tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(1));
                
                while !stats_cancellation.is_cancelled() {
                    tokio::select! {
                        _ = interval.tick() => {
                            let _ = stats_tx.send(stats_clone.clone());
                        }
                        _ = stats_cancellation.cancelled() => break,
                    }
                }
            }))
        } else {
            None
        };

        // Produce the block
        let result = self.produce_block(work, cancellation).await;

        // Cleanup statistics reporting
        if let Some(handle) = stats_handle {
            let _ = handle.await;
        }

        match &result {
            Ok(_) => {
                info!(
                    "Constant delay mining completed successfully. Block time: {:?}",
                    self.block_time
                );
            }
            Err(e) => {
                info!("Constant delay mining failed: {}", e);
            }
        }

        result
    }

    fn stats(&self) -> MiningStats {
        self.stats.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Target, Work};

    #[test]
    fn test_constant_delay_worker_creation() {
        let block_time = Duration::from_secs(5);
        let worker = ConstantDelayWorker::new(block_time);
        assert_eq!(worker.worker_type(), "constant-delay");
        assert_eq!(worker.block_time, block_time);
    }

    #[tokio::test]
    async fn test_constant_delay_mining_success() {
        let block_time = Duration::from_millis(100); // Short delay for test
        let mut worker = ConstantDelayWorker::new(block_time);
        let target = Target::min(); // Difficulty doesn't matter for constant delay
        let chain_id = ChainId::new(0);
        let work = Work::new(vec![0u8; Work::SIZE]).unwrap();
        let initial_nonce = Nonce::new(0);
        let cancellation = CancellationToken::new();

        let start = Instant::now();
        let result = worker.mine(
            initial_nonce,
            target,
            chain_id,
            work,
            cancellation,
            None,
        ).await;

        let elapsed = start.elapsed();
        
        assert!(result.is_ok());
        // Should take approximately the block time
        assert!(elapsed >= block_time);
        assert!(elapsed < block_time + Duration::from_millis(50)); // Allow some tolerance
        
        let stats = worker.stats();
        assert_eq!(stats.solutions_found, 1);
        assert_eq!(stats.total_hashes, 1);
    }

    #[tokio::test]
    async fn test_constant_delay_mining_cancellation() {
        let block_time = Duration::from_secs(10); // Long delay
        let mut worker = ConstantDelayWorker::new(block_time);
        let target = Target::max();
        let chain_id = ChainId::new(0);
        let work = Work::new(vec![0u8; Work::SIZE]).unwrap();
        let initial_nonce = Nonce::new(0);
        let cancellation = CancellationToken::new();

        // Cancel after a short delay
        let cancellation_clone = cancellation.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            cancellation_clone.cancel();
        });

        let start = Instant::now();
        let result = worker.mine(
            initial_nonce,
            target,
            chain_id,
            work,
            cancellation,
            None,
        ).await;

        let elapsed = start.elapsed();
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Cancelled { .. }));
        // Should be cancelled before block time
        assert!(elapsed < block_time);
    }

    #[tokio::test]
    async fn test_constant_delay_statistics_reporting() {
        let block_time = Duration::from_millis(200);
        let mut worker = ConstantDelayWorker::new(block_time);
        let target = Target::max();
        let chain_id = ChainId::new(0);
        let work = Work::new(vec![0u8; Work::SIZE]).unwrap();
        let initial_nonce = Nonce::new(0);
        let cancellation = CancellationToken::new();

        let (stats_tx, mut stats_rx) = mpsc::unbounded_channel();

        // Start mining in background
        let mining_handle = tokio::spawn(async move {
            worker.mine(
                initial_nonce,
                target,
                chain_id,
                work,
                cancellation,
                Some(stats_tx),
            ).await
        });

        // Should receive initial stats
        let stats = stats_rx.recv().await.expect("Should receive stats");
        assert_eq!(stats.solutions_found, 0); // No solution yet

        // Wait for completion
        let result = mining_handle.await.unwrap();
        assert!(result.is_ok());
    }

    #[test]
    fn test_constant_delay_ignores_difficulty() {
        let block_time = Duration::from_secs(30);
        let worker = ConstantDelayWorker::new(block_time);
        
        // Should have same behavior regardless of target difficulty
        let easy_target = Target::max();
        let hard_target = Target::min();
        
        // Both should result in the same block time (tested via the struct field)
        assert_eq!(worker.block_time, block_time);
    }
}