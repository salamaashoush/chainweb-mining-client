//! Mining worker implementations
//!
//! Provides different mining worker types including CPU, external command,
//! simulation, and testing workers.

use crate::{ChainId, Error, Nonce, Result, Target, Work};
use async_trait::async_trait;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::Span;

pub mod cpu;
pub mod external;
pub mod simulation;
pub mod constant_delay;
pub mod on_demand;

pub use cpu::CpuWorker;
pub use external::ExternalWorker;
pub use simulation::SimulationWorker;
pub use constant_delay::ConstantDelayWorker;
pub use on_demand::OnDemandWorker;

/// Mining statistics for a worker
#[derive(Debug, Clone, Default)]
pub struct MiningStats {
    /// Total hashes computed
    pub total_hashes: u64,
    /// Number of solutions found
    pub solutions_found: u64,
    /// Time spent mining (seconds)
    pub mining_time_secs: u64,
    /// Current hash rate (hashes per second)
    pub current_hash_rate: f64,
    /// Average hash rate (hashes per second)
    pub average_hash_rate: f64,
}

impl MiningStats {
    /// Update hash rate calculations
    pub fn update_hash_rate(&mut self, new_hashes: u64, elapsed_secs: f64) {
        self.total_hashes += new_hashes;
        self.current_hash_rate = new_hashes as f64 / elapsed_secs;
        
        if self.mining_time_secs > 0 {
            self.average_hash_rate = self.total_hashes as f64 / self.mining_time_secs as f64;
        }
    }
}

/// Mining worker trait
///
/// All mining workers must implement this trait to provide a unified interface
/// for different mining strategies.
#[async_trait]
pub trait MiningWorker: Send + Sync {
    /// Get the worker type name for logging
    fn worker_type(&self) -> &'static str;

    /// Start mining with the given parameters
    ///
    /// Returns a Work instance with the solution nonce when a valid solution is found.
    /// The worker should respect the cancellation token and stop mining when cancelled.
    async fn mine(
        &mut self,
        initial_nonce: Nonce,
        target: Target,
        chain_id: ChainId,
        mut work: Work,
        cancellation: CancellationToken,
        stats_tx: Option<mpsc::UnboundedSender<MiningStats>>,
    ) -> Result<Work>;

    /// Get current mining statistics
    fn stats(&self) -> MiningStats {
        MiningStats::default()
    }

    /// Prepare worker for mining (optional setup)
    async fn prepare(&mut self) -> Result<()> {
        Ok(())
    }

    /// Cleanup worker after mining (optional cleanup)
    async fn cleanup(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Worker factory for creating different types of mining workers
pub struct WorkerFactory;

impl WorkerFactory {
    /// Create a CPU mining worker
    pub fn create_cpu_worker(thread_count: usize) -> Box<dyn MiningWorker> {
        Box::new(CpuWorker::new(thread_count))
    }

    /// Create an external command worker
    pub fn create_external_worker(command: String) -> Box<dyn MiningWorker> {
        Box::new(ExternalWorker::new(command))
    }

    /// Create a simulation worker
    pub fn create_simulation_worker(hash_rate: f64) -> Box<dyn MiningWorker> {
        Box::new(SimulationWorker::new(hash_rate))
    }

    /// Create a constant delay worker
    pub fn create_constant_delay_worker(block_time: Duration) -> Box<dyn MiningWorker> {
        Box::new(ConstantDelayWorker::new(block_time))
    }

    /// Create an on-demand worker
    pub fn create_on_demand_worker(
        interface: String,
        port: u16,
    ) -> Result<Box<dyn MiningWorker>> {
        Ok(Box::new(OnDemandWorker::new(interface, port)?))
    }
}

/// Utility function to inject nonce and check solution
pub fn inject_nonce_and_check(
    work: &mut Work,
    nonce: Nonce,
    target: &Target,
) -> bool {
    work.inject_nonce(nonce);
    crate::crypto::Blake2sHasher::hash_meets_target(work.bytes(), target)
}

/// Utility function to compute hash rate over a time period
pub fn compute_hash_rate(hashes: u64, elapsed: Duration) -> f64 {
    if elapsed.as_secs_f64() > 0.0 {
        hashes as f64 / elapsed.as_secs_f64()
    } else {
        0.0
    }
}

/// Create a tracing span for mining operations
pub fn mining_span(worker_type: &str, chain_id: ChainId) -> Span {
    tracing::info_span!(
        "mining",
        worker_type = worker_type,
        chain_id = chain_id.value(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Target, Work};

    #[test]
    fn test_mining_stats() {
        let mut stats = MiningStats::default();
        
        // Simulate mining for 10 seconds with 1000 hashes
        stats.update_hash_rate(1000, 10.0);
        stats.mining_time_secs = 10;
        
        assert_eq!(stats.total_hashes, 1000);
        assert_eq!(stats.current_hash_rate, 100.0); // 1000/10
        assert_eq!(stats.average_hash_rate, 100.0); // 1000/10
        
        // Add more hashes
        stats.update_hash_rate(500, 5.0);
        stats.mining_time_secs = 15;
        
        assert_eq!(stats.total_hashes, 1500);
        assert_eq!(stats.current_hash_rate, 100.0); // 500/5
        assert_eq!(stats.average_hash_rate, 100.0); // 1500/15
    }

    #[test]
    fn test_inject_nonce_and_check() {
        let mut work = Work::new(vec![0u8; Work::SIZE]).unwrap();
        let nonce = Nonce::new(12345);
        let target = Target::max(); // Very easy target
        
        // Should meet the easy target
        assert!(inject_nonce_and_check(&mut work, nonce, &target));
        
        // Verify nonce was injected
        let extracted = work.extract_nonce().unwrap();
        assert_eq!(extracted, nonce);
    }

    #[test]
    fn test_compute_hash_rate() {
        let rate = compute_hash_rate(1000, Duration::from_secs(10));
        assert_eq!(rate, 100.0);
        
        let rate = compute_hash_rate(0, Duration::from_secs(10));
        assert_eq!(rate, 0.0);
        
        let rate = compute_hash_rate(1000, Duration::from_secs(0));
        assert_eq!(rate, 0.0);
    }

    #[tokio::test]
    async fn test_worker_factory() {
        let _cpu_worker = WorkerFactory::create_cpu_worker(2);
        let _ext_worker = WorkerFactory::create_external_worker("echo test".to_string());
        let _sim_worker = WorkerFactory::create_simulation_worker(1000.0);
        let _delay_worker = WorkerFactory::create_constant_delay_worker(Duration::from_secs(30));
        
        // These should create without panicking
        assert!(true);
    }
}