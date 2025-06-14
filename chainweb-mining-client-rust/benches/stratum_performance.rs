//! Performance benchmarks for Stratum protocol operations

use chainweb_mining_client::core::{Target, Work};
use criterion::{Criterion, criterion_group, criterion_main, BenchmarkId};
use std::hint::black_box;
use serde_json;
use std::collections::HashMap;

fn bench_stratum_message_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("stratum_message_parsing");
    
    // Sample Stratum messages
    let subscribe_msg = r#"{"id": 1, "method": "mining.subscribe", "params": ["cgminer/4.9.0"]}"#;
    let authorize_msg = r#"{"id": 2, "method": "mining.authorize", "params": ["username", "password"]}"#;
    let submit_msg = r#"{"id": 3, "method": "mining.submit", "params": ["username", "job_id", "extranonce2", "ntime", "nonce"]}"#;
    let notify_msg = r#"{"id": null, "method": "mining.notify", "params": ["job_id", "prevhash", "coinb1", "coinb2", ["merkle1", "merkle2"], "version", "nbits", "ntime", true]}"#;
    
    let messages = vec![
        ("subscribe", subscribe_msg),
        ("authorize", authorize_msg),
        ("submit", submit_msg),
        ("notify", notify_msg),
    ];
    
    for (name, message) in &messages {
        group.bench_with_input(BenchmarkId::new("parse_json", name), message, |b, message| {
            b.iter(|| {
                black_box(serde_json::from_str::<serde_json::Value>(message).unwrap());
            });
        });
    }
    
    group.finish();
}

fn bench_stratum_message_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("stratum_message_generation");
    
    group.bench_function("generate_job_notify", |b| {
        b.iter(|| {
            let job_id = "job_123456";
            let prevhash = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
            let coinbase1 = "01000000010000000000000000000000000000000000000000000000000000000000000000ffffffff";
            let coinbase2 = "ffffffff0100f2052a01000000434104";
            let merkle_branches = vec![
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
            ];
            let version = "20000000";
            let nbits = "1d00ffff";
            let ntime = "504e86b9";
            
            let notify = serde_json::json!({
                "id": serde_json::Value::Null,
                "method": "mining.notify",
                "params": [
                    job_id,
                    prevhash,
                    coinbase1,
                    coinbase2,
                    merkle_branches,
                    version,
                    nbits,
                    ntime,
                    true
                ]
            });
            
            black_box(serde_json::to_string(&notify).unwrap());
        });
    });
    
    group.bench_function("generate_set_difficulty", |b| {
        b.iter(|| {
            let difficulty = 1024.0;
            let set_difficulty = serde_json::json!({
                "id": serde_json::Value::Null,
                "method": "mining.set_difficulty",
                "params": [difficulty]
            });
            
            black_box(serde_json::to_string(&set_difficulty).unwrap());
        });
    });
    
    group.finish();
}

fn bench_nonce_splitting(c: &mut Criterion) {
    let mut group = c.benchmark_group("nonce_splitting");
    
    // Simulate Nonce1/Nonce2 splitting for ASIC pool mining
    group.bench_function("split_8_byte_nonce", |b| {
        b.iter(|| {
            let full_nonce = 0x123456789ABCDEF0u64;
            let nonce1 = (full_nonce >> 32) as u32;  // Upper 32 bits
            let nonce2 = full_nonce as u32;          // Lower 32 bits
            black_box((nonce1, nonce2));
        });
    });
    
    group.bench_function("combine_split_nonces", |b| {
        b.iter(|| {
            let nonce1 = 0x12345678u32;
            let nonce2 = 0x9ABCDEF0u32;
            let combined = ((nonce1 as u64) << 32) | (nonce2 as u64);
            black_box(combined);
        });
    });
    
    // Benchmark nonce space partitioning for multiple miners
    group.bench_function("partition_nonce_space", |b| {
        b.iter(|| {
            let num_miners = 8;
            let nonce_space_per_miner = u64::MAX / num_miners;
            let mut partitions = Vec::new();
            
            for i in 0..num_miners {
                let start = i * nonce_space_per_miner;
                let end = if i == num_miners - 1 {
                    u64::MAX
                } else {
                    (i + 1) * nonce_space_per_miner - 1
                };
                partitions.push((start, end));
            }
            
            black_box(partitions);
        });
    });
    
    group.finish();
}

fn bench_difficulty_adjustment(c: &mut Criterion) {
    let mut group = c.benchmark_group("difficulty_adjustment");
    
    group.bench_function("calculate_target_from_difficulty", |b| {
        b.iter(|| {
            let difficulties: [f32; 5] = [1.0, 16.0, 256.0, 4096.0, 65536.0];
            for &difficulty in &difficulties {
                // Simplified target calculation (difficulty as leading zeros)
                let leading_zeros = (difficulty.log2() as u8).min(255);
                let mut target_bytes = [0xFF; 32];
                let zero_bytes = (leading_zeros / 8) as usize;
                if zero_bytes < 32 {
                    for i in 0..zero_bytes {
                        target_bytes[i] = 0x00;
                    }
                    let remaining_bits = leading_zeros % 8;
                    if remaining_bits > 0 && zero_bytes < 32 {
                        target_bytes[zero_bytes] = 0xFF >> remaining_bits;
                    }
                }
                black_box(Target::from_bytes(target_bytes));
            }
        });
    });
    
    group.finish();
}

fn bench_job_management(c: &mut Criterion) {
    let mut group = c.benchmark_group("job_management");
    
    group.bench_function("job_storage_operations", |b| {
        b.iter(|| {
            let mut jobs: HashMap<String, serde_json::Value> = HashMap::new();
            
            // Simulate adding jobs
            for i in 0..100 {
                let job_id = format!("job_{:06}", i);
                let job = serde_json::json!({
                    "id": job_id,
                    "target": "0000ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                    "work": "000102030405060708090a0b0c0d0e0f"
                });
                jobs.insert(job_id, job);
            }
            
            // Simulate job lookups
            for i in 0..50 {
                let job_id = format!("job_{:06}", i);
                black_box(jobs.get(&job_id));
            }
            
            // Simulate job cleanup (remove old jobs)
            for i in 0..25 {
                let job_id = format!("job_{:06}", i);
                jobs.remove(&job_id);
            }
            
            black_box(jobs.len());
        });
    });
    
    group.finish();
}

fn bench_share_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("share_validation");
    
    let work = Work::from_bytes({
        let mut data = [0u8; 286];
        data[0] = 0x12;
        data[1] = 0x34;
        data
    });
    
    let targets = vec![
        ("easy", Target::from_bytes([0xFF; 32])),
        ("medium", Target::from_bytes({
            let mut bytes = [0u8; 32];
            bytes[0] = 0x00;
            bytes[1] = 0xFF;
            bytes
        })),
        ("hard", Target::from_bytes({
            let mut bytes = [0u8; 32];
            bytes[0] = 0x00;
            bytes[1] = 0x00;
            bytes[2] = 0x0F;
            bytes
        })),
    ];
    
    for (name, target) in &targets {
        group.bench_with_input(BenchmarkId::new("validate_share", name), &(work.clone(), target), |b, (work, target)| {
            b.iter(|| {
                // Simulate share validation process
                let hash = work.hash();
                let meets_target = work.meets_target(target);
                let meets_difficulty = meets_target; // Simplified
                black_box((hash, meets_target, meets_difficulty));
            });
        });
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_stratum_message_parsing,
    bench_stratum_message_generation,
    bench_nonce_splitting,
    bench_difficulty_adjustment,
    bench_job_management,
    bench_share_validation
);
criterion_main!(benches);