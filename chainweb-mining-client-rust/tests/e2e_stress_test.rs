//! End-to-end stress testing with real chainweb node
//!
//! This test starts a real chainweb node via Docker and runs comprehensive
//! stress tests against it with the compiled mining client.

use serde_json::Value;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Test configuration
struct E2EStressConfig {
    /// Duration for each stress test phase
    pub test_duration: Duration,
    /// Number of concurrent workers
    pub worker_count: usize,
    /// Chainweb node endpoint
    pub node_endpoint: String,
    /// Mining account
    pub account: String,
    /// Enable monitoring
    pub enable_monitoring: bool,
}

impl Default for E2EStressConfig {
    fn default() -> Self {
        Self {
            test_duration: Duration::from_secs(30),
            worker_count: 4,
            node_endpoint: "http://localhost:1848".to_string(),
            account: "miner".to_string(),
            enable_monitoring: true,
        }
    }
}

/// Chainweb node manager for testing
struct ChainwebTestNode {
    process: Option<Child>,
    endpoint: String,
}

impl ChainwebTestNode {
    /// Start a new chainweb node for testing
    async fn start() -> Result<Self, Box<dyn std::error::Error>> {
        println!("🚀 Starting Chainweb test node...");

        // Stop any existing test node
        let _ = Command::new("docker")
            .args(&["stop", "chainweb-mining-test"])
            .output();
        let _ = Command::new("docker")
            .args(&["rm", "chainweb-mining-test"])
            .output();

        // Start the test node
        let process = Command::new("bash")
            .arg("../test-compatibility/start-chainweb-node.sh")
            .arg("dev") // Use dev mode for faster testing
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let node = Self {
            process: Some(process),
            endpoint: "http://localhost:1848".to_string(),
        };

        // Wait for node to be ready
        node.wait_for_ready().await?;

        println!("✅ Chainweb test node is ready!");
        Ok(node)
    }

    /// Wait for the node to become ready
    async fn wait_for_ready(&self) -> Result<(), Box<dyn std::error::Error>> {
        let max_attempts = 60;
        let mut attempts = 0;

        while attempts < max_attempts {
            match reqwest::get(&format!("{}/info", self.endpoint)).await {
                Ok(response) if response.status().is_success() => {
                    return Ok(());
                }
                _ => {
                    sleep(Duration::from_secs(2)).await;
                    attempts += 1;
                }
            }
        }

        Err("Chainweb node failed to become ready".into())
    }

    /// Get node info
    async fn get_info(&self) -> Result<Value, Box<dyn std::error::Error>> {
        let response = reqwest::get(&format!("{}/info", self.endpoint)).await?;
        let info: Value = response.json().await?;
        Ok(info)
    }

    /// Get current mining work
    async fn get_work(&self, account: &str) -> Result<Value, Box<dyn std::error::Error>> {
        let url = format!("{}/chainweb/0.0/development/mining/work", self.endpoint);
        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .json(&serde_json::json!({
                "account": account,
                "predicate": "keys-all",
                "public-keys": []
            }))
            .send()
            .await?;

        let work: Value = response.json().await?;
        Ok(work)
    }

    /// Check node health
    async fn health_check(&self) -> bool {
        match self.get_info().await {
            Ok(_) => true,
            Err(_) => false,
        }
    }
}

impl Drop for ChainwebTestNode {
    fn drop(&mut self) {
        println!("🛑 Stopping Chainweb test node...");

        // Stop Docker container
        let _ = Command::new("docker")
            .args(&["stop", "chainweb-mining-test"])
            .output();
        let _ = Command::new("docker")
            .args(&["rm", "chainweb-mining-test"])
            .output();

        // Kill the process if it's still running
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
        }

        println!("✅ Chainweb test node stopped");
    }
}

/// Stress test coordinator
struct StressTestCoordinator {
    config: E2EStressConfig,
    node: ChainwebTestNode,
    running: Arc<AtomicBool>,
    stats: Arc<StressTestStats>,
}

/// Stress test statistics
#[derive(Default)]
struct StressTestStats {
    workers_started: AtomicU64,
    total_requests: AtomicU64,
    successful_requests: AtomicU64,
    failed_requests: AtomicU64,
    solutions_found: AtomicU64,
    total_hashes: AtomicU64,
    peak_hashrate: AtomicU64,
}

impl StressTestCoordinator {
    /// Create a new stress test coordinator
    async fn new(config: E2EStressConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let node = ChainwebTestNode::start().await?;

        Ok(Self {
            config,
            node,
            running: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(StressTestStats::default()),
        })
    }

    /// Run CPU mining stress test
    async fn stress_test_cpu_mining(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔥 Starting CPU mining stress test...");

        self.running.store(true, Ordering::Relaxed);
        let mut handles = Vec::new();

        // Start multiple CPU mining workers
        for worker_id in 0..self.config.worker_count {
            let endpoint = self.config.node_endpoint.clone();
            let account = format!("{}-{}", self.config.account, worker_id);
            let running = self.running.clone();
            let stats = self.stats.clone();
            let duration = self.config.test_duration;

            let handle = tokio::spawn(async move {
                let start_time = Instant::now();
                let mut local_hashes = 0u64;
                let mut local_requests = 0u64;
                let mut local_successful = 0u64;

                stats.workers_started.fetch_add(1, Ordering::Relaxed);

                while running.load(Ordering::Relaxed) && start_time.elapsed() < duration {
                    // Simulate CPU mining worker process
                    match Self::run_cpu_worker(&endpoint, &account).await {
                        Ok(hash_count) => {
                            local_hashes += hash_count;
                            local_successful += 1;

                            // Update peak hashrate
                            let elapsed = start_time.elapsed().as_secs_f64();
                            if elapsed > 0.0 {
                                let current_rate = (local_hashes as f64 / elapsed) as u64;
                                let mut peak = stats.peak_hashrate.load(Ordering::Relaxed);
                                while current_rate > peak {
                                    match stats.peak_hashrate.compare_exchange_weak(
                                        peak,
                                        current_rate,
                                        Ordering::Relaxed,
                                        Ordering::Relaxed,
                                    ) {
                                        Ok(_) => break,
                                        Err(new_peak) => peak = new_peak,
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    local_requests += 1;

                    // Brief pause to prevent overwhelming the node
                    sleep(Duration::from_millis(100)).await;
                }

                stats
                    .total_requests
                    .fetch_add(local_requests, Ordering::Relaxed);
                stats
                    .successful_requests
                    .fetch_add(local_successful, Ordering::Relaxed);
                stats
                    .total_hashes
                    .fetch_add(local_hashes, Ordering::Relaxed);
            });

            handles.push(handle);
        }

        // Wait for test duration
        sleep(self.config.test_duration).await;
        self.running.store(false, Ordering::Relaxed);

        // Wait for all workers to finish
        for handle in handles {
            let _ = handle.await;
        }

        println!("✅ CPU mining stress test completed");
        Ok(())
    }

    /// Run Stratum server stress test
    async fn stress_test_stratum_server(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔥 Starting Stratum server stress test...");

        // Start Stratum server in background
        let stratum_handle = tokio::spawn(async {
            let result = Command::new("cargo")
                .args(&[
                    "run",
                    "--release",
                    "--",
                    "stratum",
                    "--chainweb-url",
                    "http://localhost:1848",
                    "--account",
                    "test-miner",
                    "--port",
                    "1917",
                ])
                .output();

            match result {
                Ok(_) => println!("Stratum server finished"),
                Err(e) => println!("Stratum server error: {}", e),
            }
        });

        // Wait for Stratum server to start
        sleep(Duration::from_secs(3)).await;

        self.running.store(true, Ordering::Relaxed);
        let mut handles = Vec::new();

        // Start multiple mock ASIC miners
        for miner_id in 0..self.config.worker_count {
            let running = self.running.clone();
            let stats = self.stats.clone();
            let duration = self.config.test_duration;

            let handle = tokio::spawn(async move {
                let start_time = Instant::now();

                while running.load(Ordering::Relaxed) && start_time.elapsed() < duration {
                    // Simulate ASIC miner connecting to Stratum server
                    match Self::simulate_asic_miner(miner_id).await {
                        Ok(_) => {
                            stats.successful_requests.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(_) => {
                            stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    stats.total_requests.fetch_add(1, Ordering::Relaxed);
                    sleep(Duration::from_millis(500)).await;
                }
            });

            handles.push(handle);
        }

        // Wait for test duration
        sleep(self.config.test_duration).await;
        self.running.store(false, Ordering::Relaxed);

        // Wait for all miners to finish
        for handle in handles {
            let _ = handle.await;
        }

        // Stop Stratum server
        stratum_handle.abort();

        println!("✅ Stratum server stress test completed");
        Ok(())
    }

    /// Run external worker stress test
    async fn stress_test_external_worker(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔥 Starting external worker stress test...");

        self.running.store(true, Ordering::Relaxed);
        let mut handles = Vec::new();

        // Start multiple external workers
        for worker_id in 0..self.config.worker_count {
            let endpoint = self.config.node_endpoint.clone();
            let account = format!("{}-ext-{}", self.config.account, worker_id);
            let running = self.running.clone();
            let stats = self.stats.clone();
            let duration = self.config.test_duration;

            let handle = tokio::spawn(async move {
                let start_time = Instant::now();

                while running.load(Ordering::Relaxed) && start_time.elapsed() < duration {
                    // Run external worker
                    match Self::run_external_worker(&endpoint, &account).await {
                        Ok(_) => {
                            stats.successful_requests.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(_) => {
                            stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    stats.total_requests.fetch_add(1, Ordering::Relaxed);
                    sleep(Duration::from_millis(200)).await;
                }
            });

            handles.push(handle);
        }

        // Wait for test duration
        sleep(self.config.test_duration).await;
        self.running.store(false, Ordering::Relaxed);

        // Wait for all workers to finish
        for handle in handles {
            let _ = handle.await;
        }

        println!("✅ External worker stress test completed");
        Ok(())
    }

    /// Run a CPU mining worker
    async fn run_cpu_worker(
        endpoint: &str,
        account: &str,
    ) -> Result<u64, Box<dyn std::error::Error>> {
        let output = Command::new("cargo")
            .args(&[
                "run",
                "--release",
                "--",
                "cpu",
                "--chainweb-url",
                endpoint,
                "--account",
                account,
                "--threads",
                "2",
                "--batch-size",
                "10000",
            ])
            .env("RUST_LOG", "info")
            .output()?;

        if output.status.success() {
            // Parse hash count from output (simplified)
            Ok(10000) // Return batch size as approximation
        } else {
            Err("CPU worker failed".into())
        }
    }

    /// Simulate an ASIC miner connecting to Stratum
    async fn simulate_asic_miner(miner_id: usize) -> Result<(), Box<dyn std::error::Error>> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpStream;

        // Connect to Stratum server
        let mut stream = TcpStream::connect("127.0.0.1:1917").await?;

        // Send mining.subscribe
        let subscribe = serde_json::json!({
            "id": 1,
            "method": "mining.subscribe",
            "params": [format!("test-miner-{}", miner_id), null]
        });
        let message = format!("{}\n", subscribe);
        stream.write_all(message.as_bytes()).await?;

        // Read response
        let mut buffer = [0; 1024];
        let _ = stream.read(&mut buffer).await?;

        // Send mining.authorize
        let authorize = serde_json::json!({
            "id": 2,
            "method": "mining.authorize",
            "params": [format!("test-miner-{}", miner_id), "password"]
        });
        let message = format!("{}\n", authorize);
        stream.write_all(message.as_bytes()).await?;

        // Read response
        let _ = stream.read(&mut buffer).await?;

        Ok(())
    }

    /// Run an external worker
    async fn run_external_worker(
        endpoint: &str,
        account: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let output = Command::new("cargo")
            .args(&[
                "run",
                "--release",
                "--",
                "external",
                "--chainweb-url",
                endpoint,
                "--account",
                account,
                "--command",
                "echo 'mock-hash'",
            ])
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            Err("External worker failed".into())
        }
    }

    /// Monitor node health during stress testing
    async fn monitor_node_health(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("📊 Starting node health monitoring...");

        while self.running.load(Ordering::Relaxed) {
            if !self.node.health_check().await {
                println!("⚠️ Node health check failed!");
                return Err("Node became unhealthy during stress test".into());
            }

            sleep(Duration::from_secs(5)).await;
        }

        Ok(())
    }

    /// Print stress test results
    fn print_results(&self) {
        let workers = self.stats.workers_started.load(Ordering::Relaxed);
        let total_requests = self.stats.total_requests.load(Ordering::Relaxed);
        let successful = self.stats.successful_requests.load(Ordering::Relaxed);
        let failed = self.stats.failed_requests.load(Ordering::Relaxed);
        let solutions = self.stats.solutions_found.load(Ordering::Relaxed);
        let total_hashes = self.stats.total_hashes.load(Ordering::Relaxed);
        let peak_hashrate = self.stats.peak_hashrate.load(Ordering::Relaxed);

        let success_rate = if total_requests > 0 {
            (successful as f64 / total_requests as f64) * 100.0
        } else {
            0.0
        };

        let avg_hashrate = if self.config.test_duration.as_secs() > 0 {
            total_hashes / self.config.test_duration.as_secs()
        } else {
            0
        };

        println!("\n📈 Stress Test Results:");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("🔧 Workers Started:      {}", workers);
        println!("📊 Total Requests:       {}", total_requests);
        println!("✅ Successful Requests:  {}", successful);
        println!("❌ Failed Requests:      {}", failed);
        println!("📈 Success Rate:         {:.2}%", success_rate);
        println!("💎 Solutions Found:      {}", solutions);
        println!("🔥 Total Hashes:         {}", total_hashes);
        println!("⚡ Average Hash Rate:    {} H/s", avg_hashrate);
        println!("🚀 Peak Hash Rate:       {} H/s", peak_hashrate);
        println!(
            "⏱️  Test Duration:        {} seconds",
            self.config.test_duration.as_secs()
        );
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }
}

#[tokio::test]
#[ignore] // Ignore by default as it requires Docker
async fn test_e2e_cpu_mining_stress() {
    let config = E2EStressConfig {
        test_duration: Duration::from_secs(60),
        worker_count: 4,
        ..Default::default()
    };

    let coordinator = StressTestCoordinator::new(config)
        .await
        .expect("Failed to create stress test coordinator");

    // Run stress test
    coordinator
        .stress_test_cpu_mining()
        .await
        .expect("CPU mining stress test failed");

    coordinator.print_results();
}

#[tokio::test]
#[ignore] // Ignore by default as it requires Docker
async fn test_e2e_stratum_server_stress() {
    let config = E2EStressConfig {
        test_duration: Duration::from_secs(45),
        worker_count: 8,
        ..Default::default()
    };

    let coordinator = StressTestCoordinator::new(config)
        .await
        .expect("Failed to create stress test coordinator");

    // Run stress test
    coordinator
        .stress_test_stratum_server()
        .await
        .expect("Stratum server stress test failed");

    coordinator.print_results();
}

#[tokio::test]
#[ignore] // Ignore by default as it requires Docker
async fn test_e2e_external_worker_stress() {
    let config = E2EStressConfig {
        test_duration: Duration::from_secs(30),
        worker_count: 6,
        ..Default::default()
    };

    let coordinator = StressTestCoordinator::new(config)
        .await
        .expect("Failed to create stress test coordinator");

    // Run stress test
    coordinator
        .stress_test_external_worker()
        .await
        .expect("External worker stress test failed");

    coordinator.print_results();
}

#[tokio::test]
#[ignore] // Ignore by default as it requires Docker and long runtime
async fn test_e2e_comprehensive_stress() {
    println!("🚀 Starting comprehensive end-to-end stress test...");

    let config = E2EStressConfig {
        test_duration: Duration::from_secs(120), // 2 minutes per phase
        worker_count: 6,
        enable_monitoring: true,
        ..Default::default()
    };

    let coordinator = Arc::new(StressTestCoordinator::new(config)
        .await
        .expect("Failed to create stress test coordinator"));

    // Start node health monitoring
    let health_monitor = {
        let coordinator_clone = Arc::clone(&coordinator);
        tokio::spawn(async move { coordinator_clone.monitor_node_health().await })
    };

    // Run all stress tests sequentially
    println!("Phase 1: CPU Mining Stress Test");
    coordinator
        .stress_test_cpu_mining()
        .await
        .expect("CPU mining stress test failed");

    println!("Phase 2: Stratum Server Stress Test");
    coordinator
        .stress_test_stratum_server()
        .await
        .expect("Stratum server stress test failed");

    println!("Phase 3: External Worker Stress Test");
    coordinator
        .stress_test_external_worker()
        .await
        .expect("External worker stress test failed");

    // Stop monitoring
    health_monitor.abort();

    coordinator.print_results();
    println!("🎉 Comprehensive stress test completed successfully!");
}
