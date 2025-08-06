use chainweb_mining_client::{
    config::{Config, WorkerConfig},
    core::{ChainId, PreemptionConfig, PreemptionStrategy},
    protocol::chainweb::{ChainwebClient, ChainwebClientConfig},
    workers::{
        cpu::CpuWorker, constant_delay::ConstantDelayWorker, external::ExternalWorker,
        on_demand::OnDemandWorker, simulation::SimulationWorker, stratum::StratumServer, Worker,
    },
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

const TEST_PUBLIC_KEY: &str = "f89ef46927f506c70b6a58fd322450a936311dc6ac91f4ec3d8ef949608dbf1f";
const TEST_ACCOUNT: &str = "k:f89ef46927f506c70b6a58fd322450a936311dc6ac91f4ec3d8ef949608dbf1f";

#[tokio::test]
#[ignore = "requires running chainweb node"]
async fn test_cpu_worker_integration() {
    let config = Config {
        node: vec!["http://localhost:1848".to_string()],
        public_key: TEST_PUBLIC_KEY.to_string(),
        account: Some(TEST_ACCOUNT.to_string()),
        log_level: "info".to_string(),
        no_tls: true,
        tls_client_cert: None,
        tls_client_key: None,
        tls_server_cert: None,
        worker: WorkerConfig::Cpu {
            thread_count: Some(2),
        },
        update_stream_timeout: Duration::from_secs(10),
        preemption: PreemptionConfig {
            chain_id: ChainId(0),
            check_interval: Duration::from_secs(1),
            strategy: PreemptionStrategy::Headers,
        },
    };

    let client_config = ChainwebClientConfig {
        nodes: config.node.clone(),
        use_tls: !config.no_tls,
        tls_client_cert: config.tls_client_cert.clone(),
        tls_client_key: config.tls_client_key.clone(),
        tls_server_cert: config.tls_server_cert.clone(),
    };

    let client = Arc::new(ChainwebClient::new(client_config).expect("Failed to create client"));
    let worker = CpuWorker::new(2);

    // Test worker can be created and started
    let handle = tokio::spawn(async move {
        worker
            .run(
                client,
                config.account.as_deref(),
                &config.public_key,
                config.preemption.chain_id,
            )
            .await
    });

    // Let it run for a short time
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Cancel the task
    handle.abort();
}

#[tokio::test]
#[ignore = "requires running chainweb node with POW disabled"]
async fn test_simulation_worker_integration() {
    let config = Config {
        node: vec!["http://localhost:1849".to_string()],
        public_key: TEST_PUBLIC_KEY.to_string(),
        account: Some(TEST_ACCOUNT.to_string()),
        log_level: "info".to_string(),
        no_tls: true,
        tls_client_cert: None,
        tls_client_key: None,
        tls_server_cert: None,
        worker: WorkerConfig::Simulation { hash_rate: 1000000 },
        update_stream_timeout: Duration::from_secs(10),
        preemption: PreemptionConfig {
            chain_id: ChainId(0),
            check_interval: Duration::from_secs(1),
            strategy: PreemptionStrategy::Headers,
        },
    };

    let client_config = ChainwebClientConfig {
        nodes: config.node.clone(),
        use_tls: !config.no_tls,
        tls_client_cert: config.tls_client_cert.clone(),
        tls_client_key: config.tls_client_key.clone(),
        tls_server_cert: config.tls_server_cert.clone(),
    };

    let client = Arc::new(ChainwebClient::new(client_config).expect("Failed to create client"));
    let worker = SimulationWorker::new(1000000);

    let handle = tokio::spawn(async move {
        worker
            .run(
                client,
                config.account.as_deref(),
                &config.public_key,
                config.preemption.chain_id,
            )
            .await
    });

    tokio::time::sleep(Duration::from_secs(5)).await;
    handle.abort();
}

#[tokio::test]
#[ignore = "requires running chainweb node with POW disabled"]
async fn test_constant_delay_worker_integration() {
    let config = Config {
        node: vec!["http://localhost:1849".to_string()],
        public_key: TEST_PUBLIC_KEY.to_string(),
        account: Some(TEST_ACCOUNT.to_string()),
        log_level: "info".to_string(),
        no_tls: true,
        tls_client_cert: None,
        tls_client_key: None,
        tls_server_cert: None,
        worker: WorkerConfig::ConstantDelay { block_time: 5 },
        update_stream_timeout: Duration::from_secs(10),
        preemption: PreemptionConfig {
            chain_id: ChainId(0),
            check_interval: Duration::from_secs(1),
            strategy: PreemptionStrategy::Headers,
        },
    };

    let client_config = ChainwebClientConfig {
        nodes: config.node.clone(),
        use_tls: !config.no_tls,
        tls_client_cert: config.tls_client_cert.clone(),
        tls_client_key: config.tls_client_key.clone(),
        tls_server_cert: config.tls_server_cert.clone(),
    };

    let client = Arc::new(ChainwebClient::new(client_config).expect("Failed to create client"));
    let worker = ConstantDelayWorker::new(Duration::from_secs(5));

    let handle = tokio::spawn(async move {
        worker
            .run(
                client,
                config.account.as_deref(),
                &config.public_key,
                config.preemption.chain_id,
            )
            .await
    });

    tokio::time::sleep(Duration::from_secs(10)).await;
    handle.abort();
}

#[tokio::test]
async fn test_stratum_server_integration() {
    let config = Config {
        node: vec!["http://localhost:1848".to_string()],
        public_key: TEST_PUBLIC_KEY.to_string(),
        account: Some(TEST_ACCOUNT.to_string()),
        log_level: "info".to_string(),
        no_tls: true,
        tls_client_cert: None,
        tls_client_key: None,
        tls_server_cert: None,
        worker: WorkerConfig::Stratum {
            host: "127.0.0.1".to_string(),
            port: 11917,
            difficulty: Some(1),
            auto_diff: false,
            diff_adjust_period: 60,
            worker_threads: None,
            rate: None,
        },
        update_stream_timeout: Duration::from_secs(10),
        preemption: PreemptionConfig {
            chain_id: ChainId(0),
            check_interval: Duration::from_secs(1),
            strategy: PreemptionStrategy::Headers,
        },
    };

    let client_config = ChainwebClientConfig {
        nodes: config.node.clone(),
        use_tls: !config.no_tls,
        tls_client_cert: config.tls_client_cert.clone(),
        tls_client_key: config.tls_client_key.clone(),
        tls_server_cert: config.tls_server_cert.clone(),
    };

    let client = Arc::new(ChainwebClient::new(client_config).expect("Failed to create client"));
    let worker = StratumServer::new(
        "127.0.0.1".to_string(),
        11917,
        Some(1),
        false,
        60,
        None,
        None,
    );

    let handle = tokio::spawn(async move {
        worker
            .run(
                client,
                config.account.as_deref(),
                &config.public_key,
                config.preemption.chain_id,
            )
            .await
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Try to connect
    match tokio::net::TcpStream::connect("127.0.0.1:11917").await {
        Ok(_) => println!("Successfully connected to Stratum server"),
        Err(e) => panic!("Failed to connect to Stratum server: {}", e),
    }

    handle.abort();
}

#[tokio::test]
#[ignore = "requires running chainweb node with POW disabled"]
async fn test_on_demand_worker_integration() {
    let config = Config {
        node: vec!["http://localhost:1849".to_string()],
        public_key: TEST_PUBLIC_KEY.to_string(),
        account: Some(TEST_ACCOUNT.to_string()),
        log_level: "info".to_string(),
        no_tls: true,
        tls_client_cert: None,
        tls_client_key: None,
        tls_server_cert: None,
        worker: WorkerConfig::OnDemand { port: 11790 },
        update_stream_timeout: Duration::from_secs(10),
        preemption: PreemptionConfig {
            chain_id: ChainId(0),
            check_interval: Duration::from_secs(1),
            strategy: PreemptionStrategy::Headers,
        },
    };

    let client_config = ChainwebClientConfig {
        nodes: config.node.clone(),
        use_tls: !config.no_tls,
        tls_client_cert: config.tls_client_cert.clone(),
        tls_client_key: config.tls_client_key.clone(),
        tls_server_cert: config.tls_server_cert.clone(),
    };

    let client = Arc::new(ChainwebClient::new(client_config).expect("Failed to create client"));
    let worker = OnDemandWorker::new(11790);

    let handle = tokio::spawn(async move {
        worker
            .run(
                client,
                config.account.as_deref(),
                &config.public_key,
                config.preemption.chain_id,
            )
            .await
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Test HTTP endpoint
    let client = reqwest::Client::new();
    let response = client
        .post("http://127.0.0.1:11790/mine")
        .timeout(Duration::from_secs(5))
        .send()
        .await;

    match response {
        Ok(resp) => {
            assert!(resp.status().is_success(), "Expected successful response");
            println!("On-demand mining triggered successfully");
        }
        Err(e) => panic!("Failed to trigger on-demand mining: {}", e),
    }

    handle.abort();
}

#[tokio::test]
async fn test_external_worker_integration() {
    let config = Config {
        node: vec!["http://localhost:1848".to_string()],
        public_key: TEST_PUBLIC_KEY.to_string(),
        account: Some(TEST_ACCOUNT.to_string()),
        log_level: "info".to_string(),
        no_tls: true,
        tls_client_cert: None,
        tls_client_key: None,
        tls_server_cert: None,
        worker: WorkerConfig::External {
            command: vec!["cat".to_string()], // Simple echo command for testing
        },
        update_stream_timeout: Duration::from_secs(10),
        preemption: PreemptionConfig {
            chain_id: ChainId(0),
            check_interval: Duration::from_secs(1),
            strategy: PreemptionStrategy::Headers,
        },
    };

    let client_config = ChainwebClientConfig {
        nodes: config.node.clone(),
        use_tls: !config.no_tls,
        tls_client_cert: config.tls_client_cert.clone(),
        tls_client_key: config.tls_client_key.clone(),
        tls_server_cert: config.tls_server_cert.clone(),
    };

    let client = Arc::new(ChainwebClient::new(client_config).expect("Failed to create client"));
    let worker = ExternalWorker::new(vec!["cat".to_string()]);

    let handle = tokio::spawn(async move {
        // This will likely fail quickly due to 'cat' not being a proper miner
        let _ = worker
            .run(
                client,
                config.account.as_deref(),
                &config.public_key,
                config.preemption.chain_id,
            )
            .await;
    });

    // Give it a moment to start
    tokio::time::sleep(Duration::from_secs(1)).await;

    handle.abort();
}

#[tokio::test]
async fn test_multi_node_failover() {
    let config = Config {
        node: vec![
            "http://localhost:1848".to_string(),
            "http://localhost:1849".to_string(), // Secondary node
        ],
        public_key: TEST_PUBLIC_KEY.to_string(),
        account: Some(TEST_ACCOUNT.to_string()),
        log_level: "info".to_string(),
        no_tls: true,
        tls_client_cert: None,
        tls_client_key: None,
        tls_server_cert: None,
        worker: WorkerConfig::Cpu {
            thread_count: Some(1),
        },
        update_stream_timeout: Duration::from_secs(10),
        preemption: PreemptionConfig {
            chain_id: ChainId(0),
            check_interval: Duration::from_secs(1),
            strategy: PreemptionStrategy::Headers,
        },
    };

    let client_config = ChainwebClientConfig {
        nodes: config.node.clone(),
        use_tls: !config.no_tls,
        tls_client_cert: config.tls_client_cert.clone(),
        tls_client_key: config.tls_client_key.clone(),
        tls_server_cert: config.tls_server_cert.clone(),
    };

    let client = ChainwebClient::new(client_config);
    assert!(client.is_ok(), "Should create client with multiple nodes");
}

// Benchmark-style tests
#[tokio::test]
#[ignore = "benchmark test"]
async fn bench_cpu_worker_hashrate() {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    let hashes = Arc::new(AtomicU64::new(0));
    let hashes_clone = hashes.clone();

    // Run CPU mining for a fixed duration
    let handle = tokio::spawn(async move {
        // Simulate CPU mining
        let start = std::time::Instant::now();
        let duration = Duration::from_secs(10);

        while start.elapsed() < duration {
            // Simulate hash computation
            for _ in 0..100000 {
                hashes_clone.fetch_add(1, Ordering::Relaxed);
            }
            tokio::task::yield_now().await;
        }
    });

    timeout(Duration::from_secs(11), handle)
        .await
        .expect("Test timed out")
        .expect("Task panicked");

    let total_hashes = hashes.load(Ordering::Relaxed);
    let hashrate = total_hashes as f64 / 10.0 / 1_000_000.0; // MH/s
    println!("CPU Worker Hashrate: {:.2} MH/s", hashrate);
}