//! CPU mining implementation using multiple threads

use crate::core::{Nonce, Target, VectorizedMiner, Work, SimdMiner, detect_simd_features};
use crate::error::Result;
use crate::utils::monitoring::global_monitoring;
use crate::workers::{MiningResult, Worker};
use async_trait::async_trait;
#[cfg(test)]
use blake2::Digest;
use parking_lot::Mutex;
#[cfg(test)]
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

/// Reusable buffer pool for nonce batches
#[derive(Clone)]
struct NonceBufferPool {
    buffers: Arc<Mutex<Vec<Vec<u64>>>>,
    batch_size: u64,
}

impl NonceBufferPool {
    fn new(batch_size: u64, initial_capacity: usize) -> Self {
        let mut buffers = Vec::with_capacity(initial_capacity);
        for _ in 0..initial_capacity {
            let buffer = vec![0; batch_size as usize];
            buffers.push(buffer);
        }

        Self {
            buffers: Arc::new(Mutex::new(buffers)),
            batch_size,
        }
    }

    fn get_buffer(&self) -> Vec<u64> {
        let mut buffers = self.buffers.lock();
        buffers.pop().unwrap_or_else(|| {
            vec![0; self.batch_size as usize]
        })
    }

    fn return_buffer(&self, buffer: Vec<u64>) {
        if buffer.capacity() >= self.batch_size as usize {
            let mut buffers = self.buffers.lock();
            if buffers.len() < 8 {
                // Limit pool size
                buffers.push(buffer);
            }
        }
    }
}

/// CPU mining worker with SIMD optimizations
pub struct CpuWorker {
    config: CpuWorkerConfig,
    is_mining: Arc<AtomicBool>,
    hash_count: Arc<AtomicU64>,
    last_hashrate_time: Arc<Mutex<Instant>>,
    nonce_pool: NonceBufferPool,
    vectorized_miner_pool: Arc<Mutex<Vec<VectorizedMiner>>>,
    simd_miner_pool: Arc<Mutex<Vec<SimdMiner>>>,
    use_simd: bool,
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
        
        // Detect SIMD features
        let simd_features = detect_simd_features();
        let use_simd = simd_features.has_avx2 || simd_features.has_sse41 || simd_features.has_neon;
        info!("CPU features: {}", simd_features.description());
        
        if use_simd {
            info!("Using SIMD-optimized Blake2s implementation");
        } else {
            info!("Using standard Blake2s implementation");
        }

        // Configure rayon thread pool
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()
            .ok();

        // Create vectorized miners for each thread
        let mut vectorized_miners = Vec::with_capacity(threads);
        let mut simd_miners = Vec::with_capacity(threads);
        for _ in 0..threads {
            vectorized_miners.push(VectorizedMiner::new(config.batch_size as usize));
            simd_miners.push(SimdMiner::new(config.batch_size as usize));
        }

        Self {
            config: config.clone(),
            is_mining: Arc::new(AtomicBool::new(false)),
            hash_count: Arc::new(AtomicU64::new(0)),
            last_hashrate_time: Arc::new(Mutex::new(Instant::now())),
            nonce_pool: NonceBufferPool::new(config.batch_size, threads),
            vectorized_miner_pool: Arc::new(Mutex::new(vectorized_miners)),
            simd_miner_pool: Arc::new(Mutex::new(simd_miners)),
            use_simd,
        }
    }

    /// Mine a single batch of nonces with optimized memory usage
    /// This is an alternative implementation kept for testing and benchmarking
    #[cfg(test)]
    fn mine_batch_optimized(
        work_bytes: &[u8; 286], // Work as bytes to avoid cloning
        target: &Target,
        start_nonce: u64,
        nonce_buffer: &mut Vec<u64>,
        is_mining: &AtomicBool,
    ) -> Option<(Nonce, [u8; 32])> {
        // Fill buffer with nonce values (reuses existing allocation)
        for (i, nonce_val) in nonce_buffer.iter_mut().enumerate() {
            *nonce_val = start_nonce + i as u64;
        }

        nonce_buffer.par_iter().find_map_any(|&nonce_value| {
            if !is_mining.load(Ordering::Relaxed) {
                return None;
            }

            // Create working copy on stack (avoid allocation)
            let mut work_copy = *work_bytes;

            // Modify nonce in-place (nonce is at bytes 278-286)
            let nonce = Nonce::new(nonce_value);
            work_copy[crate::core::constants::NONCE_OFFSET..].copy_from_slice(&nonce.to_le_bytes());

            // Hash directly from bytes using Blake2s-256
            let mut hasher = blake2::Blake2s256::new();
            hasher.update(work_copy);
            let hash: [u8; 32] = hasher.finalize().into();
            if target.meets_target(&hash) {
                Some((nonce, hash))
            } else {
                None
            }
        })
    }

    /// SIMD-optimized batch mining using new SIMD hasher
    fn mine_batch_simd_optimized(
        work_bytes: &[u8; 286],
        target: &Target,
        start_nonce: u64,
        batch_size: u64,
        simd_miner: &mut SimdMiner,
        is_mining: &AtomicBool,
    ) -> Option<(Nonce, [u8; 32])> {
        let simd_batch_size = (batch_size as usize).min(simd_miner.batch_size());
        let num_batches = (batch_size as usize).div_ceil(simd_batch_size);

        for batch_idx in 0..num_batches {
            if !is_mining.load(Ordering::Relaxed) {
                return None;
            }

            let batch_start_nonce = start_nonce + (batch_idx * simd_batch_size) as u64;
            let current_batch_size = if batch_idx == num_batches - 1 {
                batch_size as usize - batch_idx * simd_batch_size
            } else {
                simd_batch_size
            };

            // Use SIMD mining for this batch
            if let Some((nonce, hash)) = simd_miner.mine_batch(
                work_bytes,
                target,
                batch_start_nonce,
                current_batch_size,
            ) {
                return Some((nonce, hash));
            }
        }

        None
    }

    /// SIMD-optimized batch mining using vectorized hashing
    fn mine_batch_simd(
        work_bytes: &[u8; 286],
        target: &Target,
        start_nonce: u64,
        batch_size: u64,
        vectorized_miner: &mut VectorizedMiner,
        is_mining: &AtomicBool,
    ) -> Option<(Nonce, [u8; 32])> {
        // Use adaptive batch sizing for optimal SIMD performance
        let simd_batch_size = (batch_size as usize).min(vectorized_miner.work_buffer.len());
        let num_batches = (batch_size as usize).div_ceil(simd_batch_size);

        for batch_idx in 0..num_batches {
            if !is_mining.load(Ordering::Relaxed) {
                return None;
            }

            let batch_start_nonce = start_nonce + (batch_idx * simd_batch_size) as u64;
            let current_batch_size = if batch_idx == num_batches - 1 {
                batch_size as usize - batch_idx * simd_batch_size
            } else {
                simd_batch_size
            };

            // Use vectorized mining for this sub-batch
            let hashes =
                vectorized_miner.mine_batch(work_bytes, batch_start_nonce, current_batch_size);

            // Check each hash against target
            for (i, hash) in hashes.iter().enumerate() {
                if target.meets_target(hash) {
                    let solution_nonce = batch_start_nonce + i as u64;
                    return Some((Nonce::new(solution_nonce), *hash));
                }
            }
        }

        None
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
        let nonce_pool = self.nonce_pool.clone();
        let vectorized_pool = self.vectorized_miner_pool.clone();
        let simd_pool = self.simd_miner_pool.clone();
        let use_simd = self.use_simd;

        // Get work as bytes once to avoid repeated cloning
        let work_bytes = *work.as_bytes();

        // Spawn mining task
        task::spawn_blocking(move || {
            let mut current_nonce = 0u64;
            let nonce_buffer = nonce_pool.get_buffer();

            // Get appropriate miner based on SIMD support
            let mut vectorized_miner = if !use_simd {
                Some({
                    let mut pool = vectorized_pool.lock();
                    pool.pop()
                        .unwrap_or_else(|| VectorizedMiner::new(batch_size as usize))
                })
            } else {
                None
            };
            
            let mut simd_miner = if use_simd {
                Some({
                    let mut pool = simd_pool.lock();
                    pool.pop()
                        .unwrap_or_else(|| SimdMiner::new(batch_size as usize))
                })
            } else {
                None
            };

            // Initialize monitoring
            let monitoring = global_monitoring();
            let start_time = Instant::now();
            let mut last_hash_rate_update = Instant::now();

            info!("Starting CPU mining with {} optimizations", 
                  if use_simd { "AVX2/SIMD" } else { "standard" });

            let mining_result = loop {
                if !is_mining.load(Ordering::Relaxed) {
                    break None;
                }

                // Use appropriate mining method based on SIMD support
                let mining_result = if let Some(ref mut simd_miner) = simd_miner {
                    Self::mine_batch_simd_optimized(
                        &work_bytes,
                        &target,
                        current_nonce,
                        batch_size,
                        simd_miner,
                        &is_mining,
                    )
                } else if let Some(ref mut vectorized_miner) = vectorized_miner {
                    Self::mine_batch_simd(
                        &work_bytes,
                        &target,
                        current_nonce,
                        batch_size,
                        vectorized_miner,
                        &is_mining,
                    )
                } else {
                    None
                };

                if let Some((nonce, hash)) = mining_result {
                    info!("Found solution! Nonce: {} ({})", nonce,
                          if use_simd { "AVX2/SIMD" } else { "standard" });

                    // Record solution found
                    monitoring.record_solution();

                    // Create solved work only when solution is found
                    let mut solved_work = work;
                    solved_work.set_nonce(nonce);

                    let result = MiningResult {
                        work: solved_work,
                        nonce,
                        hash,
                    };

                    // Stop mining after finding solution
                    is_mining.store(false, Ordering::Relaxed);
                    break Some(result);
                }

                // Update counters
                hash_count.fetch_add(batch_size, Ordering::Relaxed);
                current_nonce = current_nonce.wrapping_add(batch_size);

                // Update monitoring metrics periodically
                let now = Instant::now();
                if now.duration_since(last_hash_rate_update) >= Duration::from_secs(1) {
                    let total_hashes = hash_count.load(Ordering::Relaxed);
                    let elapsed = now.duration_since(start_time).as_secs_f64();
                    let current_hash_rate = if elapsed > 0.0 {
                        total_hashes as f64 / elapsed
                    } else {
                        0.0
                    };

                    monitoring.record_hash_rate(current_hash_rate);
                    last_hash_rate_update = now;
                }

                // Yield occasionally to prevent blocking
                if current_nonce % (batch_size * 100) == 0 {
                    std::thread::yield_now();
                }
            };

            // Return miners to their pools
            if let Some(vectorized_miner) = vectorized_miner {
                let mut pool = vectorized_pool.lock();
                if pool.len() < 16 {
                    // Limit pool size
                    pool.push(vectorized_miner);
                }
            }
            
            if let Some(simd_miner) = simd_miner {
                let mut pool = simd_pool.lock();
                if pool.len() < 16 {
                    // Limit pool size
                    pool.push(simd_miner);
                }
            }

            // Return buffer to pool before sending result
            nonce_pool.return_buffer(nonce_buffer);

            // Send result if found
            if let Some(result) = mining_result {
                let _ = result_tx.blocking_send(result);
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

        // Test with optimized batch mining
        let work_bytes = *work.as_bytes();
        let mut nonce_buffer = vec![0u64; 1000];

        // Should find solution in first batch
        let result =
            CpuWorker::mine_batch_optimized(&work_bytes, &target, 0, &mut nonce_buffer, &is_mining);
        assert!(result.is_some());

        if let Some((nonce, hash)) = result {
            assert!(target.meets_target(&hash));
            assert!(nonce.value() < 1000);
        }
    }
}
