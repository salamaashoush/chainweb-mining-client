//! Chainweb Mining Client
//!
//! High-performance mining client for the Kadena Chainweb network.


use chainweb_mining_client::{
    config::{Args, Config, WorkerConfig},
    core::{ChainId, PreemptionConfig, PreemptionDecision, PreemptionStrategy, WorkPreemptor},
    error::Result,
    protocol::chainweb::{ChainwebClient, ChainwebClientConfig},
    utils::{self, monitoring::global_monitoring},
    workers::{
        Worker,
        cpu::{CpuWorker, CpuWorkerConfig},
        external::{ExternalWorker, ExternalWorkerConfig},
    },
};
use clap::Parser;
use futures::StreamExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

const INFO_MESSAGE: &str = r#"
Chainweb Mining Client

A mining client for Kadena's chainweb node mining API. It supports:

- mining with ASICs through a stratum server,
- simulated mining for testing,
- multi threaded CPU mining,
- external mining workers (e.g. a GPU),
- timed miners for non-PoW usecases.

Competitive mining on the Kadena Mainnet requires special mining hardware
(ASIC), which connects to a Stratum Server from where it obtains work.

All other mining modes (GPU, CPU, and simulation) are intended only for testing.
"#;

const LONG_INFO_MESSAGE: &str = r#"
Chainweb Mining Client

Detailed information about the mining client...

For more information, visit: https://github.com/kadena-io/chainweb-mining-client
"#;

const LICENSE: &str = r#"
BSD 3-Clause License

Copyright (c) 2019-2024, Kadena LLC
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
   contributors may be used to endorse or promote products derived from
   this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
"#;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments
    let args = Args::parse();

    // Handle special info flags
    if args.info {
        println!("{}", INFO_MESSAGE);
        return Ok(());
    }

    if args.long_info {
        println!("{}", LONG_INFO_MESSAGE);
        return Ok(());
    }

    if args.show_version {
        println!("chainweb-mining-client {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if args.license {
        println!("{}", LICENSE);
        return Ok(());
    }

    // Handle monitoring status
    if args.monitoring_status {
        print_monitoring_status();
        return Ok(());
    }

    // Handle key generation
    if args.generate_key {
        generate_key_pair();
        return Ok(());
    }

    // Handle print config
    let print_config_flag = args.print_config;
    let print_config_format = args.print_config_as.clone();

    // Load configuration
    let config = Config::from_args(args)?;

    if print_config_flag || print_config_format.is_some() {
        let format = print_config_format.as_deref().unwrap_or("full");
        print_config(&config, format)?;
        return Ok(());
    }

    // Initialize logging
    utils::init_logging(&config.logging.level, &config.logging.format);

    // Initialize monitoring system
    let _monitoring = global_monitoring();
    info!("📊 Monitoring system initialized");

    info!(
        "Starting Chainweb Mining Client v{}",
        env!("CARGO_PKG_VERSION")
    );
    let chain_str = config
        .node
        .chain_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "all chains".to_string());
    info!(
        "Mining on {} for account {}",
        chain_str, config.mining.account
    );

    // Create Chainweb client
    let chainweb_config = ChainwebClientConfig {
        node_url: config.node.url.clone(),
        chain_id: config
            .node
            .chain_id
            .map(ChainId::new)
            .unwrap_or(ChainId::new(0)),
        account: config.mining.account.clone(),
        public_key: config.mining.public_key.clone(),
        timeout: Duration::from_secs(config.node.timeout_secs),
        use_tls: config.node.use_tls,
        insecure: config.node.insecure,
    };

    let mut client = ChainwebClient::new(chainweb_config)?;

    // Get node info
    let node_info = client.get_node_info().await?;
    info!(
        "Connected to node: {} (API v{})",
        node_info.node_version, node_info.node_api_version
    );

    // Set the node version for future API calls
    client.set_node_version(node_info.node_version.clone());
    
    // Create Arc for shared ownership
    let client_arc = Arc::new(client);

    // Create worker based on configuration
    let worker: Arc<dyn Worker> = match &config.worker {
        WorkerConfig::Cpu {
            threads,
            batch_size,
        } => {
            let cpu_config = CpuWorkerConfig {
                threads: *threads,
                batch_size: *batch_size,
                update_interval: Duration::from_secs(1),
            };
            Arc::new(CpuWorker::new(cpu_config))
        }
        WorkerConfig::Gpu {
            device_index,
            workgroup_size,
            workgroup_count,
            batch_size,
            enable_monitoring,
        } => {
            let gpu_config = chainweb_mining_client::workers::gpu::GpuConfig {
                device_index: *device_index,
                workgroup_size: *workgroup_size,
                workgroup_count: *workgroup_count,
                batch_size: *batch_size,
                enable_monitoring: *enable_monitoring,
            };
            let gpu_worker = tokio::runtime::Handle::current()
                .block_on(chainweb_mining_client::workers::gpu::GpuWorker::new(gpu_config))
                .expect("Failed to create GPU worker");
            Arc::new(gpu_worker)
        }
        WorkerConfig::External {
            command,
            args,
            env,
            timeout_secs,
        } => {
            let external_config = ExternalWorkerConfig {
                command: PathBuf::from(command),
                args: args.clone(),
                env: env.clone(),
                timeout_secs: *timeout_secs,
            };
            Arc::new(ExternalWorker::new(external_config))
        }
        WorkerConfig::Stratum {
            port,
            host,
            max_connections,
            difficulty,
            rate_ms,
        } => {
            let stratum_config = chainweb_mining_client::workers::stratum::StratumServerConfig {
                port: *port,
                host: host.clone(),
                max_connections: *max_connections,
                difficulty: difficulty.clone(),
                rate_ms: *rate_ms,
                authorize_callback: None, // No custom authorization by default
            };
            Arc::new(chainweb_mining_client::workers::stratum::StratumServer::new(stratum_config))
        }
        WorkerConfig::Simulation { hash_rate } => {
            let simulation_config =
                chainweb_mining_client::workers::simulation::SimulationWorkerConfig {
                    hash_rate: *hash_rate,
                };
            Arc::new(
                chainweb_mining_client::workers::simulation::SimulationWorker::new(
                    simulation_config,
                ),
            )
        }
        WorkerConfig::ConstantDelay { block_time_secs } => {
            let constant_delay_config =
                chainweb_mining_client::workers::constant_delay::ConstantDelayWorkerConfig {
                    block_time_secs: *block_time_secs,
                };
            Arc::new(
                chainweb_mining_client::workers::constant_delay::ConstantDelayWorker::new(
                    constant_delay_config,
                ),
            )
        }
        WorkerConfig::OnDemand { port, host } => {
            let on_demand_config =
                chainweb_mining_client::workers::on_demand::OnDemandWorkerConfig {
                    port: *port,
                    host: host.clone(),
                };
            Arc::new(
                chainweb_mining_client::workers::on_demand::OnDemandWorker::new(on_demand_config),
            )
        }
    };

    info!("Using {} worker", worker.worker_type());

    // Create work preemptor with default configuration
    let preemption_config = PreemptionConfig {
        strategy: PreemptionStrategy::Immediate,
        min_preemption_interval: Duration::from_millis(100),
        max_work_fetch_time: Duration::from_secs(5),
        validate_work_change: true,
    };
    let preemptor = WorkPreemptor::new(preemption_config);

    // Create channel for mining results
    let (result_tx, mut result_rx) = mpsc::channel(10);

    // Subscribe to work updates
    let mut update_stream = client_arc.subscribe_updates().await?;
    
    // Stream reconnection state
    let mut stream_retry_count = 0u32;
    let mut stream_retry_delay = Duration::from_millis(100);
    const MAX_STREAM_RETRIES: u32 = 10;
    const MAX_STREAM_DELAY: Duration = Duration::from_secs(30);

    // Get initial work
    let (mut current_work, mut current_target) = client_arc.get_work().await?;
    info!("Received initial work");

    // Start mining
    worker
        .mine(current_work.clone(), current_target, result_tx.clone())
        .await?;

    // Main mining loop
    loop {
        tokio::select! {
            // Handle mining results
            Some(result) = result_rx.recv() => {
                info!("Found solution! Nonce: {}", result.nonce);

                // Submit solution
                match client_arc.submit_solution(&result.work).await {
                    Ok(()) => {
                        info!("Solution accepted!");
                    }
                    Err(e) => {
                        error!("Failed to submit solution: {}", e);
                    }
                }

                // Get new work and continue mining
                match client_arc.get_work().await {
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

                        // Get new work first
                        match client_arc.get_work().await {
                            Ok((new_work, new_target)) => {
                                // Use preemptor to decide if and how to preempt
                                let decision = preemptor.should_preempt(&new_work, &current_work);

                                match decision {
                                    PreemptionDecision::Preempt(action) => {
                                        info!("Preempting current work with action: {:?}", action);

                                        // Execute preemption using the sophisticated logic
                                        let worker_clone = worker.clone();
                                        let result_tx_clone = result_tx.clone();
                                        let client_clone = Arc::clone(&client_arc);

                                        if let Err(e) = preemptor.execute_preemption(
                                            action,
                                            worker_clone,
                                            new_work.clone(),
                                            new_target,
                                            result_tx_clone,
                                            move || async move {
                                                // This closure can be used for re-fetching work if needed
                                                client_clone.get_work().await
                                            }
                                        ).await {
                                            error!("Failed to execute preemption: {}", e);
                                        } else {
                                            // Update current work if preemption succeeded
                                            current_work = new_work;
                                        }
                                    }
                                    PreemptionDecision::Skip(reason) => {
                                        info!("Skipping work preemption: {:?}", reason);
                                        // Continue with current work
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to get updated work: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Update stream error: {}", e);
                        
                        // Attempt to reconnect with exponential backoff
                        if stream_retry_count < MAX_STREAM_RETRIES {
                            stream_retry_count += 1;
                            info!("Attempting to reconnect stream (attempt {}/{})", stream_retry_count, MAX_STREAM_RETRIES);
                            
                            // Wait before reconnecting
                            tokio::time::sleep(stream_retry_delay).await;
                            
                            // Try to reconnect
                            match client_arc.subscribe_updates().await {
                                Ok(new_stream) => {
                                    update_stream = new_stream;
                                    info!("Successfully reconnected to update stream");
                                    
                                    // Reset retry state on successful reconnection
                                    stream_retry_count = 0;
                                    stream_retry_delay = Duration::from_millis(100);
                                }
                                Err(reconnect_error) => {
                                    error!("Failed to reconnect to updates: {}", reconnect_error);
                                    
                                    // Increase delay for next attempt (exponential backoff)
                                    stream_retry_delay = std::cmp::min(
                                        Duration::from_millis((stream_retry_delay.as_millis() as f64 * 2.0) as u64),
                                        MAX_STREAM_DELAY
                                    );
                                }
                            }
                        } else {
                            error!("Max stream reconnection attempts exceeded, giving up on automatic updates");
                            // Continue mining with current work but without updates
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

        // Print preemption statistics periodically
        let stats = preemptor.get_stats();
        if stats.total_preemptions > 0 {
            info!(
                "Preemption stats: {} total, {} skipped, avg work fetch: {:.1}ms, avg restart: {:.1}ms",
                stats.total_preemptions,
                stats.skipped_preemptions,
                stats.avg_work_fetch_time_ms,
                stats.avg_restart_time_ms
            );
        }
    }

    info!("Mining client stopped");
    Ok(())
}

/// Generate a new Ed25519 key pair for mining
fn generate_key_pair() {
    use ed25519_dalek::{SigningKey, VerifyingKey};

    let mut secret_bytes = [0u8; 32];
    getrandom::fill(&mut secret_bytes).expect("Failed to generate random bytes");
    let signing_key = SigningKey::from_bytes(&secret_bytes);
    let verifying_key: VerifyingKey = (&signing_key).into();

    let private_key = hex::encode(signing_key.to_bytes());
    let public_key = hex::encode(verifying_key.to_bytes());

    println!("public:  {}", public_key);
    println!("private: {}", private_key);
}

/// Print configuration in the specified format
fn print_config(config: &Config, format: &str) -> Result<()> {
    match format {
        "full" => {
            // Print as YAML for compatibility with Haskell version
            let yaml = serde_yaml::to_string(config).map_err(|e| {
                chainweb_mining_client::error::Error::config(format!(
                    "Failed to serialize config: {}",
                    e
                ))
            })?;
            println!("{}", yaml);
        }
        "minimal" => {
            // Print only non-default values
            // For now, just print the full config
            let yaml = serde_yaml::to_string(config).map_err(|e| {
                chainweb_mining_client::error::Error::config(format!(
                    "Failed to serialize config: {}",
                    e
                ))
            })?;
            println!("{}", yaml);
        }
        "diff" => {
            // Print only values that differ from defaults
            // For now, just print the full config
            let yaml = serde_yaml::to_string(config).map_err(|e| {
                chainweb_mining_client::error::Error::config(format!(
                    "Failed to serialize config: {}",
                    e
                ))
            })?;
            println!("{}", yaml);
        }
        _ => {
            return Err(chainweb_mining_client::error::Error::config(format!(
                "Invalid print-config-as format: {}. Must be one of: full, minimal, diff",
                format
            )));
        }
    }
    Ok(())
}

/// Print current monitoring status
fn print_monitoring_status() {
    let monitoring = global_monitoring();
    let report = monitoring.generate_status_report();
    println!("{}", report);
}
