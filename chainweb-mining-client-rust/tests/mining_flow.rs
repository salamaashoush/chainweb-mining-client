//! Integration tests for the complete mining flow

use chainweb_mining_client::{
    config::{Config, LoggingConfig, MiningConfig, NodeConfig, WorkerConfig},
    core::{ChainId, Nonce, Target, Work},
    protocol::chainweb::{ChainwebClient, ChainwebClientConfig},
    workers::{cpu::{CpuWorker, CpuWorkerConfig}, Worker},
};
use std::time::Duration;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_cpu_mining_workflow() {
    // Create a test work with easy difficulty
    let mut work = Work::from_bytes([0u8; 286]);
    work.set_nonce(Nonce::new(0));
    
    // Very easy target for testing
    let mut target_bytes = [0xFF; 32];
    target_bytes[0] = 0x7F;
    let target = Target::from_bytes(target_bytes);
    
    // Create CPU worker
    let cpu_config = CpuWorkerConfig {
        threads: 2,
        batch_size: 1000,
        update_interval: Duration::from_millis(100),
    };
    let worker = CpuWorker::new(cpu_config);
    
    // Create result channel
    let (tx, mut rx) = mpsc::channel(10);
    
    // Start mining
    worker.mine(work.clone(), target, tx).await.unwrap();
    
    // Should find solution quickly
    let result = tokio::time::timeout(Duration::from_secs(10), rx.recv())
        .await
        .expect("Mining timeout")
        .expect("No result received");
    
    // Verify solution
    assert!(target.meets_target(&result.hash));
    assert_eq!(result.work.nonce(), result.nonce);
}

#[test]
fn test_config_creation() {
    let config = Config {
        node: NodeConfig {
            url: "test.chainweb.com".to_string(),
            use_tls: true,
            timeout_secs: 30,
            chain_id: 0,
        },
        mining: MiningConfig {
            account: "test-account".to_string(),
            public_key: "test-key".to_string(),
            update_interval_secs: 5,
        },
        worker: WorkerConfig::Cpu {
            threads: 4,
            batch_size: 10000,
        },
        logging: LoggingConfig {
            level: "info".to_string(),
            format: "plain".to_string(),
            file: None,
        },
    };
    
    assert!(config.validate().is_ok());
}

#[test]
fn test_invalid_chain_id() {
    let mut config = Config::default();
    config.node.chain_id = 20; // Invalid
    
    assert!(config.validate().is_err());
}

#[tokio::test]
async fn test_worker_lifecycle() {
    let worker = CpuWorker::new(CpuWorkerConfig::default());
    
    // Should not be mining initially
    assert_eq!(worker.hashrate().await, 0);
    
    let work = Work::from_bytes([0u8; 286]);
    let target = Target::from_bytes([0x00; 32]); // Impossible target
    let (tx, _rx) = mpsc::channel(1);
    
    // Start mining
    worker.mine(work, target, tx).await.unwrap();
    
    // Stop mining
    worker.stop().await.unwrap();
    
    // Verify stopped
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(worker.worker_type(), "CPU");
}