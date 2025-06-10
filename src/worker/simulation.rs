//! Simulation worker for testing and development
//!
//! Simulates mining with a configurable hash rate without actually computing hashes.

use super::{mining_span, MiningStats, MiningWorker};
use crate::{ChainId, Error, Nonce, Result, Target, Work};
use async_trait::async_trait;
use rand::Rng;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::{interval, sleep};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

/// Simulation worker that mimics mining behavior without actual computation
pub struct SimulationWorker {
    hash_rate: f64, // Hashes per second
    stats: MiningStats,
}

impl SimulationWorker {
    /// Create a new simulation worker with specified hash rate
    pub fn new(hash_rate: f64) -> Self {
        info!("Creating simulation worker with hash rate: {:.2} H/s", hash_rate);
        
        Self {
            hash_rate: hash_rate.max(1.0), // Ensure minimum rate
            stats: MiningStats::default(),
        }
    }

    /// Calculate expected time to find a solution based on difficulty
    fn calculate_expected_solution_time(&self, target: &Target) -> Duration {
        // Estimate difficulty based on target
        let difficulty_level = target.difficulty_level();
        
        // Rough approximation: 2^difficulty_level hashes needed on average
        let expected_hashes = if difficulty_level < 64 {
            1u64 << difficulty_level
        } else {
            u64::MAX // Very high difficulty
        };

        let expected_seconds = expected_hashes as f64 / self.hash_rate;
        
        // Cap at reasonable maximum
        let max_seconds = 3600.0; // 1 hour
        let capped_seconds = expected_seconds.min(max_seconds);
        
        Duration::from_secs_f64(capped_seconds)
    }

    /// Simulate finding a solution with random variation
    async fn simulate_mining(
        &mut self,
        target: Target,
        mut work: Work,
        cancellation: CancellationToken,
    ) -> Result<Work> {
        let expected_time = self.calculate_expected_solution_time(&target);
        
        // Add some randomness (±50% variation)
        let mut rng = rand::thread_rng();
        let variation_factor = rng.gen_range(0.5..1.5);
        let actual_time = Duration::from_secs_f64(expected_time.as_secs_f64() * variation_factor);
        
        info!(
            "Simulating mining for {:.2} seconds (expected: {:.2}s, difficulty level: {})",
            actual_time.as_secs_f64(),
            expected_time.as_secs_f64(),
            target.difficulty_level()
        );

        let start_time = Instant::now();
        let mut last_update = start_time;
        let mut total_hashes = 0u64;

        // Simulate mining process with regular updates
        let update_interval = Duration::from_millis(100);
        let mut update_timer = interval(update_interval);

        loop {
            tokio::select! {
                _ = update_timer.tick() => {
                    let elapsed = last_update.elapsed();
                    let hashes_this_period = (self.hash_rate * elapsed.as_secs_f64()) as u64;
                    total_hashes += hashes_this_period;
                    last_update = Instant::now();

                    // Update statistics
                    let total_elapsed = start_time.elapsed();
                    self.stats.total_hashes = total_hashes;
                    self.stats.mining_time_secs = total_elapsed.as_secs();
                    self.stats.current_hash_rate = self.hash_rate;
                    self.stats.average_hash_rate = if total_elapsed.as_secs_f64() > 0.0 {
                        total_hashes as f64 / total_elapsed.as_secs_f64()
                    } else {
                        0.0
                    };

                    debug!(
                        "Simulation progress: {:.1}s elapsed, {} hashes, {:.2} MH/s",
                        total_elapsed.as_secs_f64(),
                        total_hashes,
                        self.hash_rate / 1_000_000.0
                    );

                    // Check if we've "found" a solution
                    if total_elapsed >= actual_time {
                        // Generate a random nonce for the solution
                        let solution_nonce = Nonce::new(rng.gen());
                        work.inject_nonce(solution_nonce);
                        
                        self.stats.solutions_found = 1;
                        
                        info!(
                            "Simulation found solution after {:.2}s with nonce {}",
                            total_elapsed.as_secs_f64(),
                            solution_nonce
                        );
                        
                        return Ok(work);
                    }
                }
                _ = cancellation.cancelled() => {
                    info!("Simulation mining cancelled");
                    return Err(Error::cancelled("Simulation mining"));
                }
            }
        }
    }
}

#[async_trait]
impl MiningWorker for SimulationWorker {
    fn worker_type(&self) -> &'static str {
        "simulation"
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
            "Starting simulation mining for chain {} at {:.2} H/s (difficulty level: {})",
            chain_id,
            self.hash_rate,
            target.difficulty_level()
        );

        // Reset statistics
        self.stats = MiningStats::default();

        // Statistics reporting task
        let mut stats_clone = self.stats.clone();
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

        // Run the simulation
        let result = self.simulate_mining(target, work, cancellation).await;

        // Cleanup statistics reporting
        if let Some(handle) = stats_handle {
            let _ = handle.await;
        }

        match &result {
            Ok(_) => {
                info!(
                    "Simulation mining completed successfully. Total hashes: {}, Time: {}s",
                    self.stats.total_hashes,
                    self.stats.mining_time_secs
                );
            }
            Err(e) => {
                info!("Simulation mining failed: {}", e);
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
    fn test_simulation_worker_creation() {
        let worker = SimulationWorker::new(1000.0);
        assert_eq!(worker.worker_type(), "simulation");
        assert_eq!(worker.hash_rate, 1000.0);
    }

    #[test]
    fn test_simulation_worker_minimum_rate() {
        let worker = SimulationWorker::new(0.0);
        assert_eq!(worker.hash_rate, 1.0); // Should be clamped to minimum
    }

    #[test]
    fn test_expected_solution_time_calculation() {
        let worker = SimulationWorker::new(1000.0); // 1000 H/s
        
        // Easy target should have short expected time
        let easy_target = Target::new([u64::MAX >> 8, u64::MAX, u64::MAX, u64::MAX]);
        let easy_time = worker.calculate_expected_solution_time(&easy_target);
        assert!(easy_time < Duration::from_secs(10));
        
        // Very easy target
        let very_easy_target = Target::max();
        let very_easy_time = worker.calculate_expected_solution_time(&very_easy_target);
        assert!(very_easy_time < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_simulation_mining_success() {
        let mut worker = SimulationWorker::new(1_000_000.0); // High hash rate for fast test
        let target = Target::new([u64::MAX >> 4, u64::MAX, u64::MAX, u64::MAX]); // Easy target
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

        assert!(result.is_ok());
        assert!(start.elapsed() < Duration::from_secs(5)); // Should complete quickly
        
        let stats = worker.stats();
        assert!(stats.total_hashes > 0);
        assert_eq!(stats.solutions_found, 1);
    }

    #[tokio::test]
    async fn test_simulation_mining_cancellation() {
        let mut worker = SimulationWorker::new(1000.0);
        let target = Target::min(); // Very hard target (long mining time)
        let chain_id = ChainId::new(0);
        let work = Work::new(vec![0u8; Work::SIZE]).unwrap();
        let initial_nonce = Nonce::new(0);
        let cancellation = CancellationToken::new();

        // Cancel after a short delay
        let cancellation_clone = cancellation.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(100)).await;
            cancellation_clone.cancel();
        });

        let result = worker.mine(
            initial_nonce,
            target,
            chain_id,
            work,
            cancellation,
            None,
        ).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Cancelled { .. }));
    }

    #[tokio::test]
    async fn test_simulation_statistics_reporting() {
        let mut worker = SimulationWorker::new(10_000.0);
        let target = Target::new([u64::MAX >> 8, u64::MAX, u64::MAX, u64::MAX]);
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

        // Collect at least one statistics update
        let stats = stats_rx.recv().await.expect("Should receive stats");
        assert!(stats.current_hash_rate > 0.0);

        // Cancel and wait for completion
        let _ = mining_handle.await;
    }
}