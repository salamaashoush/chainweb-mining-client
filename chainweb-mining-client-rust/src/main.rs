//! Chainweb Mining Client
//!
//! High-performance mining client for the Kadena Chainweb network.

use chainweb_mining_client::{
    config::{Args, Config, WorkerConfig},
    core::ChainId,
    error::Result,
    protocol::chainweb::{ChainwebClient, ChainwebClientConfig},
    utils,
    workers::{cpu::{CpuWorker, CpuWorkerConfig}, external::{ExternalWorker, ExternalWorkerConfig}, Worker},
};
use clap::Parser;
use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments
    let args = Args::parse();
    
    // Load configuration
    let config = Config::from_args(args)?;
    
    // Initialize logging
    utils::init_logging(&config.logging.level, &config.logging.format);
    
    info!("Starting Chainweb Mining Client v{}", env!("CARGO_PKG_VERSION"));
    info!("Mining on chain {} for account {}", config.node.chain_id, config.mining.account);
    
    // Create Chainweb client
    let chainweb_config = ChainwebClientConfig {
        node_url: config.node.url.clone(),
        chain_id: ChainId::new(config.node.chain_id),
        account: config.mining.account.clone(),
        public_key: config.mining.public_key.clone(),
        timeout: Duration::from_secs(config.node.timeout_secs),
        use_tls: config.node.use_tls,
    };
    
    let client = ChainwebClient::new(chainweb_config)?;
    
    // Get node info
    let node_info = client.get_node_info().await?;
    info!("Connected to node: {} (API v{})", node_info.node_version, node_info.node_api_version);
    
    // Create worker based on configuration
    let worker: Arc<dyn Worker> = match &config.worker {
        WorkerConfig::Cpu { threads, batch_size } => {
            let cpu_config = CpuWorkerConfig {
                threads: *threads,
                batch_size: *batch_size,
                update_interval: Duration::from_secs(1),
            };
            Arc::new(CpuWorker::new(cpu_config))
        }
        WorkerConfig::External { command, args, env, timeout_secs } => {
            let external_config = ExternalWorkerConfig {
                command: command.clone(),
                args: args.clone(),
                env: env.clone(),
                timeout_secs: *timeout_secs,
            };
            Arc::new(ExternalWorker::new(external_config))
        }
        WorkerConfig::Stratum { .. } => {
            // TODO: Implement Stratum server
            return Err(chainweb_mining_client::error::Error::config("Stratum server not yet implemented"));
        }
    };
    
    info!("Using {} worker", worker.worker_type());
    
    // Create channel for mining results
    let (result_tx, mut result_rx) = mpsc::channel(10);
    
    // Subscribe to work updates
    let mut update_stream = client.subscribe_updates().await?;
    
    // Get initial work
    let (mut current_work, mut current_target) = client.get_work().await?;
    info!("Received initial work");
    
    // Start mining
    worker.mine(current_work.clone(), current_target, result_tx.clone()).await?;
    
    // Main mining loop
    loop {
        tokio::select! {
            // Handle mining results
            Some(result) = result_rx.recv() => {
                info!("Found solution! Nonce: {}", result.nonce);
                
                // Submit solution
                match client.submit_solution(&result.work).await {
                    Ok(()) => {
                        info!("Solution accepted!");
                    }
                    Err(e) => {
                        error!("Failed to submit solution: {}", e);
                    }
                }
                
                // Get new work and continue mining
                match client.get_work().await {
                    Ok((work, target)) => {
                        current_work = work;
                        current_target = target;
                        worker.mine(current_work.clone(), current_target, result_tx.clone()).await?;
                    }
                    Err(e) => {
                        error!("Failed to get new work: {}", e);
                    }
                }
            }
            
            // Handle work updates
            Some(update_result) = update_stream.next() => {
                match update_result {
                    Ok(_) => {
                        info!("Received work update");
                        
                        // Stop current mining
                        worker.stop().await?;
                        
                        // Get new work
                        match client.get_work().await {
                            Ok((work, target)) => {
                                current_work = work;
                                current_target = target;
                                
                                // Start mining with new work
                                worker.mine(current_work.clone(), current_target, result_tx.clone()).await?;
                            }
                            Err(e) => {
                                error!("Failed to get updated work: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Update stream error: {}", e);
                        // Reconnect to update stream
                        match client.subscribe_updates().await {
                            Ok(stream) => {
                                update_stream = stream;
                                info!("Reconnected to update stream");
                            }
                            Err(e) => {
                                error!("Failed to reconnect to updates: {}", e);
                            }
                        }
                    }
                }
            }
            
            // Handle shutdown signal
            _ = tokio::signal::ctrl_c() => {
                info!("Shutting down...");
                worker.stop().await?;
                break;
            }
        }
        
        // Print hashrate periodically
        let hashrate = worker.hashrate().await;
        if hashrate > 0 {
            info!("Current hashrate: {}", utils::format_hashrate(hashrate));
        }
    }
    
    info!("Mining client stopped");
    Ok(())
}
