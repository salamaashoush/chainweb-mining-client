//! CPU mining implementation using multiple threads

use crate::core::{Nonce, Target, Work};
use crate::error::Result;
use crate::workers::{MiningResult, Worker};
use async_trait::async_trait;
use parking_lot::Mutex;
use rayon::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task;
use tracing::{debug, info};

/// CPU mining worker configuration
#[derive(Debug, Clone)]
pub struct CpuWorkerConfig {
    /// Number of threads to use (0 = all available cores)
    pub threads: usize,
    /// Nonces to check per batch
    pub batch_size: u64,
    /// Update interval for hashrate calculation
    pub update_interval: Duration,
}

impl Default for CpuWorkerConfig {
    fn default() -> Self {
        Self {
            threads: 0, // Use all cores
            batch_size: 100_000,
            update_interval: Duration::from_secs(1),
        }
    }
}

/// CPU mining worker
pub struct CpuWorker {
    config: CpuWorkerConfig,
    is_mining: Arc<AtomicBool>,
    hash_count: Arc<AtomicU64>,
    last_hashrate_time: Arc<Mutex<Instant>>,
}

impl CpuWorker {
    /// Create a new CPU worker
    pub fn new(config: CpuWorkerConfig) -> Self {
        let threads = if config.threads == 0 {
            num_cpus::get()
        } else {
            config.threads
        };

        info!("Initializing CPU worker with {} threads", threads);

        // Configure rayon thread pool
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()
            .ok();

        Self {
            config,
            is_mining: Arc::new(AtomicBool::new(false)),
            hash_count: Arc::new(AtomicU64::new(0)),
            last_hashrate_time: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Mine a single batch of nonces
    fn mine_batch(
        work: &Work,
        target: &Target,
        start_nonce: u64,
        batch_size: u64,
        is_mining: &AtomicBool,
    ) -> Option<(Nonce, [u8; 32])> {
        let nonces: Vec<u64> = (start_nonce..start_nonce + batch_size).collect();

        nonces.par_iter().find_map_any(|&nonce_value| {
            if !is_mining.load(Ordering::Relaxed) {
                return None;
            }

            let mut test_work = work.clone();
            let nonce = Nonce::new(nonce_value);
            test_work.set_nonce(nonce);

            let hash = test_work.hash();
            if target.meets_target(&hash) {
                Some((nonce, hash))
            } else {
                None
            }
        })
    }
}

#[async_trait]
impl Worker for CpuWorker {
    async fn mine(
        &self,
        work: Work,
        target: Target,
        result_tx: mpsc::Sender<MiningResult>,
    ) -> Result<()> {
        if self.is_mining.load(Ordering::Relaxed) {
            return Err(crate::error::Error::worker("Already mining"));
        }

        self.is_mining.store(true, Ordering::Relaxed);
        self.hash_count.store(0, Ordering::Relaxed);
        *self.last_hashrate_time.lock() = Instant::now();

        let is_mining = self.is_mining.clone();
        let hash_count = self.hash_count.clone();
        let batch_size = self.config.batch_size;

        // Spawn mining task
        task::spawn_blocking(move || {
            let mut current_nonce = 0u64;

            info!("Starting CPU mining");

            while is_mining.load(Ordering::Relaxed) {
                if let Some((nonce, hash)) =
                    Self::mine_batch(&work, &target, current_nonce, batch_size, &is_mining)
                {
                    info!("Found solution! Nonce: {}", nonce);

                    let mut solved_work = work.clone();
                    solved_work.set_nonce(nonce);

                    let result = MiningResult {
                        work: solved_work,
                        nonce,
                        hash,
                    };

                    // Send result (ignore if receiver dropped)
                    let _ = result_tx.blocking_send(result);

                    // Stop mining after finding solution
                    is_mining.store(false, Ordering::Relaxed);
                    break;
                }

                // Update counters
                hash_count.fetch_add(batch_size, Ordering::Relaxed);
                current_nonce = current_nonce.wrapping_add(batch_size);

                // Yield occasionally to prevent blocking
                if current_nonce % (batch_size * 100) == 0 {
                    std::thread::yield_now();
                }
            }

            debug!("CPU mining stopped");
        });

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.is_mining.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn worker_type(&self) -> &str {
        "CPU"
    }

    async fn hashrate(&self) -> u64 {
        let hashes = self.hash_count.load(Ordering::Relaxed);
        let elapsed = self.last_hashrate_time.lock().elapsed();

        if elapsed.as_secs() == 0 {
            return 0;
        }

        // Reset counters for next measurement
        self.hash_count.store(0, Ordering::Relaxed);
        *self.last_hashrate_time.lock() = Instant::now();

        hashes / elapsed.as_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::constants::WORK_SIZE;

    #[tokio::test]
    async fn test_cpu_worker_creation() {
        let config = CpuWorkerConfig::default();
        let worker = CpuWorker::new(config);
        assert_eq!(worker.worker_type(), "CPU");
        assert!(!worker.is_mining.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_cpu_worker_mining() {
        let config = CpuWorkerConfig {
            threads: 2,
            batch_size: 1000,
            update_interval: Duration::from_millis(100),
        };
        let worker = CpuWorker::new(config);

        // Create easy work that will find solution quickly
        let work = Work::from_bytes([0u8; WORK_SIZE]);

        // Very easy target (high value means easy difficulty)
        let mut target_bytes = [0xFFu8; 32];
        target_bytes[0] = 0x7F; // Make it slightly harder than max
        let target = Target::from_bytes(target_bytes);

        let (tx, mut rx) = mpsc::channel(1);

        // Start mining
        worker.mine(work.clone(), target, tx).await.unwrap();

        // Should find solution quickly
        tokio::time::timeout(Duration::from_secs(5), async {
            if let Some(result) = rx.recv().await {
                assert!(target.meets_target(&result.hash));
                assert_eq!(result.work.nonce(), result.nonce);
            } else {
                panic!("No solution found");
            }
        })
        .await
        .unwrap();

        // Worker should stop after finding solution
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!worker.is_mining.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_cpu_worker_stop() {
        let worker = CpuWorker::new(CpuWorkerConfig::default());

        // Very hard target (won't find solution)
        let target = Target::from_bytes([0x00; 32]);
        let work = Work::from_bytes([0u8; WORK_SIZE]);
        let (tx, _rx) = mpsc::channel(1);

        // Start mining
        worker.mine(work, target, tx).await.unwrap();
        assert!(worker.is_mining.load(Ordering::Relaxed));

        // Stop mining
        worker.stop().await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!worker.is_mining.load(Ordering::Relaxed));
    }

    #[test]
    fn test_mine_batch() {
        let work = Work::from_bytes([0u8; WORK_SIZE]);

        // Easy target
        let mut target_bytes = [0xFFu8; 32];
        target_bytes[0] = 0x00;
        let target = Target::from_bytes(target_bytes);

        let is_mining = AtomicBool::new(true);

        // Should find solution in first batch
        let result = CpuWorker::mine_batch(&work, &target, 0, 1000, &is_mining);
        assert!(result.is_some());

        if let Some((nonce, hash)) = result {
            assert!(target.meets_target(&hash));
            assert!(nonce.value() < 1000);
        }
    }
}
