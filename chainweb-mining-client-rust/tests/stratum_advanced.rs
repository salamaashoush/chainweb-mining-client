//! Advanced Stratum protocol integration tests
//! 
//! Tests the complete implementation including Nonce1/Nonce2 splitting,
//! job management, and client worker identification.

use chainweb_mining_client::core::{ChainId, Target, Work};
use chainweb_mining_client::workers::stratum::{
    ClientWorker, JobId, JobManager, MiningJob, Nonce1, Nonce2, NonceSize,
    compose_nonce, split_nonce,
};

#[test]
fn test_complete_nonce_workflow() {
    // Test the complete nonce workflow for ASIC mining
    let nonce1_size = NonceSize::new(4).unwrap();
    let nonce2_size = NonceSize::new(4).unwrap();
    
    // Pool derives Nonce1 for client
    let client_id = "miner123.worker1";
    let session_salt = 0x12345678u64;
    let nonce1 = Nonce1::derive(nonce1_size, client_id, session_salt).unwrap();
    
    // Miner starts with Nonce2 = 0
    let mut nonce2 = Nonce2::new(nonce2_size, 0).unwrap();
    
    // Compose initial nonce
    let _initial_nonce = compose_nonce(nonce1, nonce2).unwrap();
    
    // Miner increments nonce2 for mining
    for i in 1..=1000 {
        assert!(nonce2.increment());
        let current_nonce = compose_nonce(nonce1, nonce2).unwrap();
        
        // Verify we can split it back
        let (split_nonce1, split_nonce2) = split_nonce(current_nonce, nonce1_size).unwrap();
        assert_eq!(split_nonce1, nonce1);
        assert_eq!(split_nonce2.value(), i);
    }
}

#[test]
fn test_job_manager_workflow() {
    let manager = JobManager::new(100, 300); // 100 jobs max, 5 minutes expiry
    
    // Create multiple jobs for different chains
    let mut job_ids = Vec::new();
    for chain_id in 0..5 {
        let job_id = manager.next_job_id();
        let work = Work::from_bytes({
            let mut data = [0u8; 286];
            data[0] = chain_id as u8; // Make each work unique
            data
        });
        let target = Target::from_bytes([0xFF; 32]);
        let nonce1 = Nonce1::new(
            NonceSize::new(4).unwrap(),
            0x12345678 + chain_id as u64
        ).unwrap();
        
        let job = MiningJob::new(
            job_id.clone(),
            ChainId::new(chain_id),
            target,
            work,
            nonce1,
            false,
        );
        
        manager.add_job(job);
        job_ids.push(job_id);
    }
    
    assert_eq!(manager.job_count(), 5);
    
    // Retrieve jobs and verify they're correct
    for (i, job_id) in job_ids.iter().enumerate() {
        let job = manager.get_job(job_id).unwrap();
        assert_eq!(job.chain_id.value(), i as u16);
        assert_eq!(job.nonce1.value(), 0x12345678 + i as u64);
    }
    
    // Test clean_jobs functionality
    let clean_job_id = manager.next_job_id();
    let clean_job = MiningJob::new(
        clean_job_id.clone(),
        ChainId::new(10),
        Target::from_bytes([0xEE; 32]),
        Work::from_bytes([0xFF; 286]),
        Nonce1::new(NonceSize::new(4).unwrap(), 0xDEADBEEF).unwrap(),
        true, // clean_jobs = true
    );
    
    manager.add_job(clean_job);
    // Adding a clean job should clear all previous jobs
    assert_eq!(manager.job_count(), 1);
    
    let remaining_job = manager.get_job(&clean_job_id).unwrap();
    assert_eq!(remaining_job.chain_id.value(), 10);
}

#[test]
fn test_client_worker_identification() {
    // Test client worker parsing and identification
    let test_cases = vec![
        ("alice", "alice", None),
        ("bob.worker1", "bob", Some("worker1")),
        ("miner123.gpu_rig_01", "miner123", Some("gpu_rig_01")),
        ("k:abcd1234.asic_farm", "k:abcd1234", Some("asic_farm")),
        ("user.with.multiple.dots", "user", Some("with.multiple.dots")),
    ];
    
    for (input, expected_user, expected_worker) in test_cases {
        let worker = ClientWorker::from_username(input);
        assert_eq!(worker.username, expected_user);
        assert_eq!(worker.worker_id, expected_worker.map(|s| s.to_string()));
        assert_eq!(worker.to_username(), input);
    }
}

#[test]
fn test_job_id_hex_encoding() {
    // Test job ID hex encoding/decoding
    let test_values = vec![0, 1, 15, 16, 255, 256, 4095, 4096, 65535, 65536];
    
    for value in test_values {
        let job_id = JobId::new(value);
        let hex_string = job_id.as_str();
        
        // Verify hex format
        assert_eq!(format!("{:x}", value), hex_string);
        
        // Verify parsing back
        let parsed_job_id = JobId::from_hex(hex_string).unwrap();
        assert_eq!(parsed_job_id, job_id);
        assert_eq!(parsed_job_id.to_u64().unwrap(), value);
    }
}

#[test]
fn test_nonce_size_validation() {
    // Test nonce size constraints
    for size in 0..=8 {
        let nonce_size = NonceSize::new(size).unwrap();
        assert_eq!(nonce_size.as_bytes(), size);
        assert_eq!(nonce_size.complement().as_bytes(), 8 - size);
        
        if size == 0 {
            assert_eq!(nonce_size.max_value(), 0);
        } else if size >= 8 {
            assert_eq!(nonce_size.max_value(), u64::MAX);
        } else {
            assert_eq!(nonce_size.max_value(), (1u64 << (size * 8)) - 1);
        }
    }
    
    // Test invalid sizes
    for invalid_size in 9..=20 {
        assert!(NonceSize::new(invalid_size).is_err());
    }
}

#[test]
fn test_stratum_notify_parameters() {
    // Test conversion to Stratum notify parameters
    let job_id = JobId::new(0x123);
    let chain_id = ChainId::new(5);
    let target = Target::from_bytes([0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
                                   0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
                                   0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
                                   0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F]);
    let work = Work::from_bytes({
        let mut data = [0u8; 286];
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }
        data
    });
    let nonce1 = Nonce1::new(NonceSize::new(4).unwrap(), 0x87654321).unwrap();
    
    let job = MiningJob::new(job_id, chain_id, target, work, nonce1, true);
    let params = job.to_notify_params();
    
    // Verify parameters structure
    let params_array = params.as_array().unwrap();
    assert_eq!(params_array.len(), 6);
    
    // Job ID
    assert_eq!(params_array[0].as_str().unwrap(), "123");
    
    // Work (hex encoded)
    let work_hex = params_array[1].as_str().unwrap();
    assert_eq!(work_hex.len(), 286 * 2); // 286 bytes = 572 hex chars
    
    // Target (hex encoded) 
    let target_hex = params_array[2].as_str().unwrap();
    assert_eq!(target_hex.len(), 32 * 2); // 32 bytes = 64 hex chars
    
    // Chain ID (hex encoded little-endian)
    let chain_id_hex = params_array[3].as_str().unwrap();
    assert_eq!(chain_id_hex, "0500"); // 5 in little-endian hex
    
    // Nonce1 (hex encoded)
    let nonce1_hex = params_array[4].as_str().unwrap();
    assert_eq!(nonce1_hex, "87654321");
    
    // Clean jobs flag
    assert_eq!(params_array[5].as_bool().unwrap(), true);
}

#[test]
fn test_concurrent_job_management() {
    use std::sync::Arc;
    use std::thread;
    
    let manager = Arc::new(JobManager::new(1000, 300));
    let mut handles = Vec::new();
    
    // Spawn multiple threads adding jobs concurrently
    for thread_id in 0..10 {
        let manager_clone = Arc::clone(&manager);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let job_id = manager_clone.next_job_id();
                let job = MiningJob::new(
                    job_id,
                    ChainId::new((thread_id * 100 + i) % 20),
                    Target::from_bytes([0xFF; 32]),
                    Work::from_bytes([0x42; 286]),
                    Nonce1::new(NonceSize::new(4).unwrap(), thread_id as u64 * 1000 + i as u64).unwrap(),
                    false,
                );
                manager_clone.add_job(job);
            }
        });
        handles.push(handle);
    }
    
    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }
    
    // Verify we have the expected number of jobs (may be less due to max_jobs limit)
    let job_count = manager.job_count();
    assert!(job_count > 0);
    assert!(job_count <= 1000); // Shouldn't exceed max_jobs
    
    // Verify job statistics
    let stats = manager.get_stats();
    assert_eq!(stats.total_jobs, job_count);
    assert!(stats.average_age_seconds >= 0.0);
}

#[test]
fn test_nonce_overflow_handling() {
    // Test nonce2 overflow behavior
    let nonce2_size = NonceSize::new(1).unwrap(); // Only 1 byte = 0-255
    let mut nonce2 = Nonce2::new(nonce2_size, 254).unwrap();
    
    // Should be able to increment once more
    assert!(nonce2.increment());
    assert_eq!(nonce2.value(), 255);
    
    // Should not be able to increment beyond max
    assert!(!nonce2.increment());
    assert_eq!(nonce2.value(), 255); // Should stay at max
    
    // Test with different sizes
    for size in 1..=4 {
        let nonce_size = NonceSize::new(size).unwrap();
        let max_value = nonce_size.max_value();
        let mut nonce = Nonce2::new(nonce_size, max_value - 1).unwrap();
        
        assert!(nonce.increment());
        assert_eq!(nonce.value(), max_value);
        assert!(!nonce.increment());
        assert_eq!(nonce.value(), max_value);
    }
}

#[test]
fn test_job_statistics_accuracy() {
    let manager = JobManager::new(10, 60);
    
    // Add jobs with slight delays to create age differences
    let mut job_ids = Vec::new();
    for i in 0..5 {
        let job_id = manager.next_job_id();
        let job = MiningJob::new(
            job_id.clone(),
            ChainId::new(i),
            Target::from_bytes([0xFF; 32]),
            Work::from_bytes([0x42; 286]),
            Nonce1::new(NonceSize::new(4).unwrap(), i as u64).unwrap(),
            false,
        );
        manager.add_job(job);
        job_ids.push(job_id);
        
        // Small delay to create age differences (in tests this is minimal)
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    
    let stats = manager.get_stats();
    assert_eq!(stats.total_jobs, 5);
    assert!(stats.average_age_seconds >= 0.0);
    // oldest_age_seconds is always >= 0 (u64), so no need to check
    assert!(stats.oldest_age_seconds >= stats.average_age_seconds as u64);
}