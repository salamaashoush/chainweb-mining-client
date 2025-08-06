//! Worker implementations for different mining strategies
//!
//! This module provides various worker types that implement different mining
//! strategies, including CPU mining, GPU mining, Stratum protocol support, and more.

use crate::core::{Nonce, Target, Work};
use crate::error::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

pub mod constant_delay;
pub mod cpu;
pub mod external;
pub mod gpu;
pub mod on_demand;
pub mod simulation;
pub mod stratum;

pub use constant_delay::ConstantDelayWorker;
pub use cpu::CpuWorker;
pub use external::ExternalWorker;
pub use gpu::GpuWorker;
pub use on_demand::OnDemandWorker;
pub use simulation::SimulationWorker;
pub use stratum::StratumServer;

/// Result of a mining operation
#[derive(Debug, Clone)]
pub struct MiningResult {
    /// The work that was solved
    pub work: Work,
    /// The winning nonce
    pub nonce: Nonce,
    /// The resulting hash
    pub hash: [u8; 32],
}

/// Trait for all worker implementations
#[async_trait]
pub trait Worker: Send + Sync {
    /// Start mining with the given work and target
    async fn mine(
        &self,
        work: Work,
        target: Target,
        result_tx: mpsc::Sender<MiningResult>,
    ) -> Result<()>;

    /// Stop the current mining operation
    async fn stop(&self) -> Result<()>;

    /// Get the worker type name
    fn worker_type(&self) -> &str;

    /// Get current hashrate (hashes per second)
    async fn hashrate(&self) -> u64;
}

/// Available worker types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerType {
    /// CPU mining using multiple threads
    Cpu,
    /// GPU mining using external GPU process
    Gpu,
    /// External worker (e.g., GPU miner)
    External,
    /// Stratum server for ASIC miners
    Stratum,
    /// Simulated mining for testing
    Simulation,
    /// Constant delay block generation
    ConstantDelay,
    /// On-demand mining via HTTP
    OnDemand,
}

impl WorkerType {
    /// Get all available worker types
    pub fn all() -> &'static [WorkerType] {
        &[
            WorkerType::Cpu,
            WorkerType::Gpu,
            WorkerType::External,
            WorkerType::Stratum,
            WorkerType::Simulation,
            WorkerType::ConstantDelay,
            WorkerType::OnDemand,
        ]
    }

    /// Get the name of the worker type
    pub fn name(&self) -> &'static str {
        match self {
            WorkerType::Cpu => "cpu",
            WorkerType::Gpu => "gpu",
            WorkerType::External => "external",
            WorkerType::Stratum => "stratum",
            WorkerType::Simulation => "simulation",
            WorkerType::ConstantDelay => "constant-delay",
            WorkerType::OnDemand => "on-demand",
        }
    }

    /// Parse a worker type from a string
    pub fn parse_worker_type(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cpu" => Some(WorkerType::Cpu),
            "gpu" => Some(WorkerType::Gpu),
            "external" => Some(WorkerType::External),
            "stratum" => Some(WorkerType::Stratum),
            "simulation" => Some(WorkerType::Simulation),
            "constant-delay" => Some(WorkerType::ConstantDelay),
            "on-demand" => Some(WorkerType::OnDemand),
            _ => None,
        }
    }
}

impl std::fmt::Display for WorkerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_type_name() {
        assert_eq!(WorkerType::Cpu.name(), "cpu");
        assert_eq!(WorkerType::External.name(), "external");
        assert_eq!(WorkerType::Stratum.name(), "stratum");
    }

    #[test]
    fn test_worker_type_from_str() {
        assert_eq!(WorkerType::parse_worker_type("cpu"), Some(WorkerType::Cpu));
        assert_eq!(WorkerType::parse_worker_type("CPU"), Some(WorkerType::Cpu));
        assert_eq!(
            WorkerType::parse_worker_type("external"),
            Some(WorkerType::External)
        );
        assert_eq!(WorkerType::parse_worker_type("invalid"), None);
    }

    #[test]
    fn test_worker_type_all() {
        let all = WorkerType::all();
        assert_eq!(all.len(), 7);
        assert!(all.contains(&WorkerType::Cpu));
        assert!(all.contains(&WorkerType::Gpu));
        assert!(all.contains(&WorkerType::External));
    }

    #[test]
    fn test_mining_result() {
        let work = Work::from_bytes([0u8; 286]);
        let nonce = Nonce::new(12345);
        let hash = [0u8; 32];

        let result = MiningResult { work, nonce, hash };
        assert_eq!(result.nonce.value(), 12345);
    }
}
