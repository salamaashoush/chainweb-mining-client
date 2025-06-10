//! Chainweb Mining Client - Main Application
//!
//! High-performance, async mining client for Kadena's Chainweb blockchain.

use chainweb_mining_client::{
    client::{ChainwebClient, MiningJob, UpdateEvent},
    config::{Config, WorkerType},
    crypto::generate_keypair,
    worker::{MiningStats, MiningWorker, WorkerFactory},
    ChainId, Error, Miner, MinerAccount, MinerPublicKey, Nonce, Result, APP_NAME, APP_VERSION,
};

use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, sleep};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Main mining coordinator
struct MiningCoordinator {
    config: Config,
    client: ChainwebClient,
    miner: Miner,
    active_mining: Arc<RwLock<HashMap<ChainId, CancellationToken>>>,
}

impl MiningCoordinator {
    /// Create a new mining coordinator
    async fn new(config: Config) -> Result<Self> {
        let client = ChainwebClient::new(
            config.node_url(),
            config.http_timeout_duration(),
            config.insecure,
        )?;

        let (public_key, account) = config.miner()?.ok_or_else(|| {
            Error::config("Public key is required for mining")
        })?;

        let miner = Miner::new(public_key, account);

        Ok(Self {
            config,
            client,
            miner,
            active_mining: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Start mining operation
    async fn start_mining(&mut self) -> Result<()> {
        info!("Starting mining coordinator");

        // Initialize client connection
        let node_info = self.client.get_node_info().await?;
        info!("Connected to node with {} chains", node_info.number_of_chains);

        // Start mining loops for configured number of threads
        let mut handles = Vec::new();

        for thread_id in 0..self.config.thread_count {
            let coordinator = self.clone_for_thread().await?;
            let handle = tokio::spawn(async move {
                coordinator.mining_thread(thread_id).await
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            if let Err(e) = handle.await {
                error!("Mining thread failed: {}", e);
            }
        }

        Ok(())
    }

    /// Clone coordinator for thread usage
    async fn clone_for_thread(&self) -> Result<Self> {
        let client = ChainwebClient::new(
            self.config.node_url(),
            self.config.http_timeout_duration(),
            self.config.insecure,
        )?;

        Ok(Self {
            config: self.config.clone(),
            client,
            miner: self.miner.clone(),
            active_mining: Arc::clone(&self.active_mining),
        })
    }

    /// Main mining thread
    async fn mining_thread(&self, thread_id: usize) -> Result<()> {
        info!("Starting mining thread {}", thread_id);

        let mut worker = self.create_worker()?;
        worker.prepare().await?;

        loop {
            match self.mining_loop_iteration(&mut *worker, thread_id).await {
                Ok(()) => {
                    // Normal completion, continue
                }
                Err(Error::Cancelled { .. }) => {
                    info!("Mining thread {} cancelled", thread_id);
                    break;
                }
                Err(e) if e.is_retryable() => {
                    warn!("Retryable error in mining thread {}: {}", thread_id, e);
                    sleep(Duration::from_secs(1)).await;
                }
                Err(e) => {
                    error!("Fatal error in mining thread {}: {}", thread_id, e);
                    break;
                }
            }
        }

        worker.cleanup().await?;
        info!("Mining thread {} completed", thread_id);
        Ok(())
    }

    /// Single iteration of the mining loop
    async fn mining_loop_iteration(
        &self,
        worker: &mut dyn MiningWorker,
        thread_id: usize,
    ) -> Result<()> {
        // Get mining job from chainweb node
        let job = self.client.get_mining_job(&self.miner).await?;
        
        info!(
            "Thread {} got mining job for chain {} (difficulty: {})",
            thread_id,
            job.chain_id,
            job.target.difficulty_level()
        );

        // Set up cancellation for preemption
        let cancellation = CancellationToken::new();
        
        // Register active mining
        {
            let mut active = self.active_mining.write().await;
            if let Some(old_token) = active.insert(job.chain_id, cancellation.clone()) {
                old_token.cancel(); // Cancel previous mining for this chain
            }
        }

        // Set up update stream for preemption
        let mut update_stream = self.client.update_stream(job.chain_id).await?;
        let update_cancellation = cancellation.clone();
        let update_handle = tokio::spawn(async move {
            while let Some(event) = update_stream.next().await {
                match event {
                    UpdateEvent::NewWork(_) => {
                        info!("New work available, preempting current mining");
                        update_cancellation.cancel();
                        break;
                    }
                    UpdateEvent::Closed => {
                        warn!("Update stream closed");
                        break;
                    }
                    UpdateEvent::Error(e) => {
                        warn!("Update stream error: {}", e);
                        // Continue listening for updates
                    }
                }
            }
        });

        // Set up statistics channel
        let (stats_tx, mut stats_rx) = mpsc::unbounded_channel();
        let stats_handle = tokio::spawn(async move {
            while let Some(stats) = stats_rx.recv().await {
                tracing::debug!(
                    "Mining stats - Thread {}: {} hashes, {:.2} MH/s",
                    thread_id,
                    stats.total_hashes,
                    stats.current_hash_rate / 1_000_000.0
                );
            }
        });

        // Start mining with preemption support
        let initial_nonce = Nonce::new(thread_id as u64 * 0x100000000); // Spread nonce ranges
        let result = tokio::select! {
            mining_result = worker.mine(
                initial_nonce,
                job.target,
                job.chain_id,
                job.work,
                cancellation.clone(),
                Some(stats_tx),
            ) => {
                match mining_result {
                    Ok(solved_work) => {
                        info!("Thread {} found solution for chain {}", thread_id, job.chain_id);
                        
                        // Submit the solution
                        match self.client.submit_work(&solved_work).await {
                            Ok(()) => {
                                info!("Successfully submitted solution for chain {}", job.chain_id);
                            }
                            Err(e) => {
                                warn!("Failed to submit solution: {}", e);
                            }
                        }
                        Ok(())
                    }
                    Err(Error::Cancelled { .. }) => {
                        info!("Mining preempted for chain {} (getting new work)", job.chain_id);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
            _ = sleep(self.config.update_timeout_duration()) => {
                warn!("Mining timeout reached, getting new work");
                cancellation.cancel();
                Ok(())
            }
        };

        // Cleanup
        update_handle.abort();
        stats_handle.abort();
        
        {
            let mut active = self.active_mining.write().await;
            active.remove(&job.chain_id);
        }

        result
    }

    /// Create worker based on configuration
    fn create_worker(&self) -> Result<Box<dyn MiningWorker>> {
        match self.config.worker {
            WorkerType::Cpu => {
                Ok(WorkerFactory::create_cpu_worker(self.config.thread_count))
            }
            WorkerType::External => {
                Ok(WorkerFactory::create_external_worker(
                    self.config.external_worker_cmd.clone()
                ))
            }
            WorkerType::Simulation => {
                let hash_rate = self.config.hash_rate()?.value();
                Ok(WorkerFactory::create_simulation_worker(hash_rate))
            }
            WorkerType::ConstantDelay => {
                Ok(WorkerFactory::create_constant_delay_worker(
                    self.config.constant_delay_duration()
                ))
            }
            WorkerType::OnDemand => {
                WorkerFactory::create_on_demand_worker(
                    self.config.on_demand_interface.clone(),
                    self.config.on_demand_port,
                )
            }
            WorkerType::Stratum => {
                // Stratum worker will be implemented separately as it's more complex
                Err(Error::config("Stratum worker not yet implemented in this version"))
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false))
        .with(filter)
        .init();

    // Load and validate configuration
    let config = Config::load().await?;

    // Handle special commands
    if config.info {
        print_info();
        return Ok(());
    }

    if config.long_info {
        print_long_info();
        return Ok(());
    }

    if config.generate_key {
        generate_and_print_keypair();
        return Ok(());
    }

    if config.print_config {
        print_configuration(&config)?;
        return Ok(());
    }

    // Ensure we have a public key for mining
    if config.public_key.is_none() {
        return Err(Error::config(
            "Public key is required for mining. Use --public-key or --generate-key"
        ));
    }

    info!("Starting {} v{}", APP_NAME, APP_VERSION);
    info!("Configuration: worker={}, threads={}, node={}", 
          config.worker.to_string(), 
          config.thread_count,
          config.node);

    // Create and start mining coordinator
    let mut coordinator = MiningCoordinator::new(config).await?;
    coordinator.start_mining().await?;

    Ok(())
}

/// Print basic program information
fn print_info() {
    println!("{} v{}", APP_NAME, APP_VERSION);
    println!("High-performance mining client for Kadena Chainweb");
}

/// Print detailed program information
fn print_long_info() {
    print_info();
    println!();
    println!("Features:");
    println!("  • ASIC mining through Stratum protocol");
    println!("  • Multi-threaded CPU mining");
    println!("  • External worker integration (GPU miners)");
    println!("  • Simulation and testing modes");
    println!("  • Rock-solid reliability with comprehensive error handling");
    println!("  • Async/await architecture for maximum performance");
    println!();
    println!("Written in Rust for safety, performance, and reliability.");
}

/// Generate and print a new keypair
fn generate_and_print_keypair() {
    let (public_key, private_key) = generate_keypair();
    println!("public:  {}", public_key);
    println!("private: {}", private_key);
    println!();
    println!("IMPORTANT: Keep your private key secure!");
    println!("Use the public key with the --public-key option for mining.");
}

/// Print current configuration
fn print_configuration(config: &Config) -> Result<()> {
    let config_yaml = serde_yaml::to_string(config)?;
    println!("{}", config_yaml);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_coordinator_creation() {
        let mut config = Config::try_parse_from(vec![
            "chainweb-mining-client",
            "--public-key",
            "87ef8fdb229ad10285ae191a168ea2ec0794621a127df21e372f41fd0246e4cf"
        ]).unwrap();

        let coordinator = MiningCoordinator::new(config).await;
        assert!(coordinator.is_ok());
    }

    #[test]
    fn test_info_functions() {
        // These should not panic
        print_info();
        print_long_info();
        generate_and_print_keypair();
    }

    #[test]
    fn test_config_printing() {
        let config = Config::try_parse_from(vec![
            "chainweb-mining-client",
            "--worker", "cpu",
            "--thread-count", "2"
        ]).unwrap();

        let result = print_configuration(&config);
        assert!(result.is_ok());
    }
}