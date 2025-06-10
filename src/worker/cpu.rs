//! CPU mining worker implementation
//!
//! High-performance multi-threaded CPU mining using Blake2s hashing with
//! optimized nonce iteration and target checking.

use super::{compute_hash_rate, inject_nonce_and_check, mining_span, MiningStats, MiningWorker};
use crate::{ChainId, Error, Nonce, Result, Target, Work};
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// CPU mining worker using multiple threads
pub struct CpuWorker {
    thread_count: usize,
    stats: Arc<CpuMiningStats>,
}

/// Thread-safe mining statistics for CPU worker
#[derive(Debug)]
struct CpuMiningStats {
    total_hashes: AtomicU64,
    solutions_found: AtomicU64,
    start_time: Instant,
    is_mining: AtomicBool,
}

impl CpuMiningStats {
    fn new() -> Self {
        Self {
            total_hashes: AtomicU64::new(0),
            solutions_found: AtomicU64::new(0),
            start_time: Instant::now(),
            is_mining: AtomicBool::new(false),
        }
    }

    fn reset(&self) {
        self.total_hashes.store(0, Ordering::Relaxed);
        self.solutions_found.store(0, Ordering::Relaxed);
        self.is_mining.store(false, Ordering::Relaxed);
    }

    fn to_mining_stats(&self) -> MiningStats {
        let total_hashes = self.total_hashes.load(Ordering::Relaxed);
        let solutions = self.solutions_found.load(Ordering::Relaxed);
        let elapsed = self.start_time.elapsed();
        let elapsed_secs = elapsed.as_secs_f64();

        MiningStats {
            total_hashes,
            solutions_found: solutions,
            mining_time_secs: elapsed.as_secs(),
            current_hash_rate: if elapsed_secs > 0.0 {
                total_hashes as f64 / elapsed_secs
            } else {
                0.0
            },
            average_hash_rate: if elapsed_secs > 0.0 {
                total_hashes as f64 / elapsed_secs
            } else {
                0.0
            },
        }
    }
}

impl CpuWorker {
    /// Create a new CPU worker with specified thread count
    pub fn new(thread_count: usize) -> Self {
        let thread_count = if thread_count == 0 {
            num_cpus::get()
        } else {
            thread_count
        };

        info!("Creating CPU worker with {} threads", thread_count);

        Self {
            thread_count,
            stats: Arc::new(CpuMiningStats::new()),
        }
    }

    /// Mine using a single thread
    async fn mine_thread(
        thread_id: usize,
        initial_nonce: Nonce,
        target: Target,
        work: Work,
        stats: Arc<CpuMiningStats>,
        cancellation: CancellationToken,
        solution_tx: mpsc::UnboundedSender<Work>,
    ) -> Result<()> {
        debug!("Starting mining thread {}", thread_id);

        // Each thread gets a different nonce range to avoid collisions
        let thread_nonce_offset = (thread_id as u64) << 48; // Use upper 16 bits for thread ID
        let mut nonce = Nonce::new(initial_nonce.value().wrapping_add(thread_nonce_offset));
        let mut work = work;
        let mut hashes_computed = 0u64;
        let mut last_stats_update = Instant::now();

        // Pre-compute target words for fast checking
        let target_bytes = target.to_bytes();
        let target_words = unsafe {
            std::mem::transmute::<[u8; 32], [u64; 4]>(
                target_bytes.try_into().expect("Target should be 32 bytes")
            )
        };

        // Mining loop with batched processing
        const BATCH_SIZE: u64 = 100_000;
        let mut batch_start = Instant::now();

        loop {
            // Check for cancellation periodically
            if cancellation.is_cancelled() {
                debug!("Thread {} cancelled", thread_id);
                break;
            }

            // Mine a batch of nonces
            for _ in 0..BATCH_SIZE {
                // Inject nonce and check target
                work.inject_nonce(nonce);
                
                // Fast hash computation and target check
                let hash = crate::crypto::Blake2sHasher::hash(work.bytes());
                if crate::crypto::fast_check_target(&target_words, &hash) {
                    info!("Solution found by thread {} with nonce {}", thread_id, nonce);
                    stats.solutions_found.fetch_add(1, Ordering::Relaxed);
                    
                    // Send solution (ignore if receiver dropped)
                    let _ = solution_tx.send(work.clone());
                    return Ok(());
                }

                nonce.increment();
                hashes_computed += 1;
            }

            // Update statistics
            let batch_elapsed = batch_start.elapsed();
            stats.total_hashes.fetch_add(BATCH_SIZE, Ordering::Relaxed);
            
            // Log progress periodically
            if last_stats_update.elapsed() >= Duration::from_secs(10) {
                let hash_rate = compute_hash_rate(BATCH_SIZE, batch_elapsed);
                debug!(
                    "Thread {} - Hash rate: {:.2} MH/s, Total hashes: {}",
                    thread_id,
                    hash_rate / 1_000_000.0,
                    hashes_computed
                );
                last_stats_update = Instant::now();
            }

            batch_start = Instant::now();

            // Yield to allow other tasks to run
            if hashes_computed % (BATCH_SIZE * 10) == 0 {
                tokio::task::yield_now().await;
            }
        }

        debug!("Thread {} completed with {} hashes", thread_id, hashes_computed);
        Ok(())
    }
}

#[async_trait]
impl MiningWorker for CpuWorker {
    fn worker_type(&self) -> &'static str {
        "cpu"
    }

    async fn mine(
        &mut self,
        initial_nonce: Nonce,
        target: Target,
        chain_id: ChainId,
        work: Work,
        cancellation: CancellationToken,
        stats_tx: Option<mpsc::UnboundedSender<MiningStats>>,
    ) -> Result<Work> {
        let _span = mining_span(self.worker_type(), chain_id);
        
        info!(
            "Starting CPU mining with {} threads for chain {} (difficulty level: {})",
            self.thread_count,
            chain_id,
            target.difficulty_level()
        );

        self.stats.reset();
        self.stats.is_mining.store(true, Ordering::Relaxed);

        // Channel for solutions from mining threads
        let (solution_tx, mut solution_rx) = mpsc::unbounded_channel();

        // Spawn mining threads
        let mut handles = Vec::new();
        for thread_id in 0..self.thread_count {
            let stats = Arc::clone(&self.stats);
            let work_clone = work.clone();
            let target_clone = target;
            let initial_nonce_clone = initial_nonce;
            let cancellation_clone = cancellation.clone();
            let solution_tx_clone = solution_tx.clone();

            let handle = task::spawn(async move {
                Self::mine_thread(
                    thread_id,
                    initial_nonce_clone,
                    target_clone,
                    work_clone,
                    stats,
                    cancellation_clone,
                    solution_tx_clone,
                ).await
            });

            handles.push(handle);
        }

        // Drop the original sender so the channel closes when all threads finish
        drop(solution_tx);

        // Statistics reporting loop
        let stats_clone = Arc::clone(&self.stats);
        let stats_cancellation = cancellation.clone();
        let stats_handle = if let Some(stats_tx) = stats_tx {
            Some(task::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(5));
                
                while !stats_cancellation.is_cancelled() {
                    tokio::select! {
                        _ = interval.tick() => {
                            let stats = stats_clone.to_mining_stats();
                            let _ = stats_tx.send(stats);
                        }
                        _ = stats_cancellation.cancelled() => break,
                    }
                }
            }))
        } else {
            None
        };

        // Wait for solution or cancellation
        let result = tokio::select! {
            solution = solution_rx.recv() => {
                match solution {
                    Some(solved_work) => {
                        info!("CPU mining found solution");
                        Ok(solved_work)
                    }
                    None => {
                        warn!("All mining threads completed without finding solution");
                        Err(Error::worker("cpu", "No solution found"))
                    }
                }
            }
            _ = cancellation.cancelled() => {
                info!("CPU mining cancelled");
                Err(Error::cancelled("CPU mining"))
            }
        };

        // Cleanup: cancel all threads and wait for completion
        cancellation.cancel();
        
        for handle in handles {
            let _ = handle.await;
        }

        if let Some(handle) = stats_handle {
            let _ = handle.await;
        }

        self.stats.is_mining.store(false, Ordering::Relaxed);

        let final_stats = self.stats.to_mining_stats();
        info!(
            "CPU mining completed. Total hashes: {}, Hash rate: {:.2} MH/s",
            final_stats.total_hashes,
            final_stats.average_hash_rate / 1_000_000.0
        );

        result
    }

    fn stats(&self) -> MiningStats {
        self.stats.to_mining_stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Target, Work};

    #[tokio::test]
    async fn test_cpu_worker_creation() {
        let worker = CpuWorker::new(2);
        assert_eq!(worker.thread_count, 2);
        assert_eq!(worker.worker_type(), "cpu");
    }

    #[tokio::test]
    async fn test_cpu_worker_easy_mining() {
        let mut worker = CpuWorker::new(1);
        let initial_nonce = Nonce::new(0);
        let target = Target::max(); // Very easy target
        let chain_id = ChainId::new(0);
        let work = Work::new(vec![0u8; Work::SIZE]).unwrap();
        let cancellation = CancellationToken::new();

        // This should find a solution quickly with the easy target
        let result = worker.mine(
            initial_nonce,
            target,
            chain_id,
            work,
            cancellation,
            None,
        ).await;

        assert!(result.is_ok());
    }

    #[test]
    fn test_cpu_mining_stats() {
        let stats = CpuMiningStats::new();
        
        stats.total_hashes.store(1000, Ordering::Relaxed);
        stats.solutions_found.store(1, Ordering::Relaxed);
        
        let mining_stats = stats.to_mining_stats();
        assert_eq!(mining_stats.total_hashes, 1000);
        assert_eq!(mining_stats.solutions_found, 1);
    }

    #[tokio::test]
    async fn test_cpu_worker_cancellation() {
        let mut worker = CpuWorker::new(1);
        let initial_nonce = Nonce::new(0);
        let target = Target::min(); // Very hard target
        let chain_id = ChainId::new(0);
        let work = Work::new(vec![0u8; Work::SIZE]).unwrap();
        let cancellation = CancellationToken::new();

        // Cancel immediately
        cancellation.cancel();

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
}