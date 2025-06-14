//! Comprehensive performance benchmarks for production readiness
//!
//! This benchmark suite provides thorough testing of all critical performance
//! characteristics needed for production deployment.

use chainweb_mining_client::core::{ChainId, Nonce, Target, Work, VectorizedMiner, AdaptiveHasher};
use chainweb_mining_client::protocol::http_pool::{HttpClientPool, HttpPoolConfig, ClientType};
use chainweb_mining_client::utils::units;
use criterion::{
    Criterion, criterion_group, criterion_main, BenchmarkId, Throughput,
    black_box, BatchSize, PlotConfiguration, AxisScale
};
use std::hint::black_box as std_black_box;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

/// Benchmark SIMD-optimized mining operations
fn bench_vectorized_mining(c: &mut Criterion) {
    let mut group = c.benchmark_group("vectorized_mining");
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));
    
    let base_work = [0x42u8; 286];
    let batch_sizes = vec![8, 16, 32, 64, 128, 256];
    
    for &batch_size in &batch_sizes {
        group.throughput(Throughput::Elements(batch_size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("vectorized_batch_mining", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.iter_batched(
                    || VectorizedMiner::new(batch_size),
                    |mut miner| {
                        let hashes = miner.mine_batch(&base_work, 0, batch_size);
                        black_box(hashes);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    
    group.finish();
}

/// Benchmark adaptive hashing performance
fn bench_adaptive_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("adaptive_hashing");
    group.measurement_time(Duration::from_secs(10));
    
    let base_work = [0x55u8; 286];
    
    group.bench_function("adaptive_hasher_auto_tune", |b| {
        b.iter_batched(
            || AdaptiveHasher::new(),
            |mut hasher| {
                hasher.auto_tune(&base_work);
                let optimal_size = hasher.optimal_batch_size();
                black_box(optimal_size);
            },
            BatchSize::SmallInput,
        );
    });
    
    group.bench_function("adaptive_hasher_optimal_performance", |b| {
        let mut hasher = AdaptiveHasher::new();
        hasher.auto_tune(&base_work);
        let optimal_size = hasher.optimal_batch_size();
        
        b.iter(|| {
            // Simulate mining with optimal batch size
            let mut vectorized = VectorizedMiner::new(optimal_size);
            let hashes = vectorized.mine_batch(&base_work, 0, optimal_size / 2);
            black_box(hashes);
        });
    });
    
    group.finish();
}

/// Stress test mining operations under high load
fn bench_stress_mining(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress_mining");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(50);
    
    // Stress test parameters
    let iterations = vec![1_000, 10_000, 100_000];
    let batch_size = 100_000;
    
    for &iteration_count in &iterations {
        group.throughput(Throughput::Elements(iteration_count));
        
        group.bench_with_input(
            BenchmarkId::new("stress_hash_computation", iteration_count),
            &iteration_count,
            |b, &iteration_count| {
                let mut work = Work::from_bytes([0x77u8; 286]);
                
                b.iter(|| {
                    let mut nonce_val = 0u64;
                    for _ in 0..iteration_count {
                        work.set_nonce(Nonce::new(nonce_val));
                        let hash = work.hash();
                        std_black_box(hash);
                        nonce_val = nonce_val.wrapping_add(1);
                    }
                });
            },
        );
    }
    
    // Memory pressure test
    group.bench_function("memory_pressure_test", |b| {
        b.iter(|| {
            let mut miners = Vec::new();
            // Create many miners to test memory allocation patterns
            for _ in 0..100 {
                miners.push(VectorizedMiner::new(batch_size));
            }
            
            // Use all miners simultaneously
            for (i, miner) in miners.iter_mut().enumerate() {
                let mut base_work = [0u8; 286];
                base_work[0] = i as u8; // Make each miner's work unique
                let hashes = miner.mine_batch(&base_work, i as u64 * 1000, 50);
                std_black_box(hashes);
            }
            
            std_black_box(miners.len());
        });
    });
    
    group.finish();
}

/// Benchmark HTTP pool performance under load
fn bench_http_pool_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("http_pool_stress");
    group.measurement_time(Duration::from_secs(10));
    
    // Test concurrent client creation and usage
    let thread_counts = vec![1, 5, 10, 20, 50];
    
    for &thread_count in &thread_counts {
        group.bench_with_input(
            BenchmarkId::new("concurrent_client_creation", thread_count),
            &thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let pool = HttpClientPool::new();
                    let handles: Vec<_> = (0..thread_count)
                        .map(|_| {
                            std::thread::spawn({
                                let pool = &pool;
                                move || {
                                    for _ in 0..10 {
                                        let _client = pool.get_client(ClientType::Mining).unwrap();
                                        let _config_client = pool.get_client(ClientType::Config).unwrap();
                                    }
                                }
                            })
                        })
                        .collect();
                    
                    for handle in handles {
                        handle.join().unwrap();
                    }
                    
                    let stats = pool.get_stats();
                    std_black_box(stats);
                });
            },
        );
    }
    
    // Test pool metrics performance
    group.bench_function("pool_metrics_overhead", |b| {
        let mut config = HttpPoolConfig::default();
        config.enable_metrics = true;
        let pool = HttpClientPool::with_config(config);
        
        b.iter(|| {
            for _ in 0..1000 {
                let _client = pool.get_client(ClientType::Mining).unwrap();
            }
            let stats = pool.get_stats();
            std_black_box(stats);
        });
    });
    
    group.finish();
}

/// Benchmark target arithmetic performance
fn bench_target_arithmetic_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("target_arithmetic_stress");
    
    // Test with various target patterns
    let target_patterns = vec![
        ("max_target", [0xFF; 32]),
        ("min_target", [0x00; 32]),
        ("medium_target", {
            let mut bytes = [0x00; 32];
            bytes[0] = 0x00;
            bytes[1] = 0xFF;
            bytes
        }),
        ("pattern_target", {
            let mut bytes = [0x00; 32];
            for (i, byte) in bytes.iter_mut().enumerate() {
                *byte = (i % 256) as u8;
            }
            bytes
        }),
    ];
    
    let works: Vec<Work> = (0..1000)
        .map(|i| {
            let mut bytes = [0u8; 286];
            bytes[0] = (i % 256) as u8;
            bytes[1] = ((i / 256) % 256) as u8;
            Work::from_bytes(bytes)
        })
        .collect();
    
    for (pattern_name, target_bytes) in &target_patterns {
        let target = Target::from_bytes(*target_bytes);
        
        group.bench_with_input(
            BenchmarkId::new("batch_target_checking", pattern_name),
            &(&works, &target),
            |b, (works, target)| {
                b.iter(|| {
                    let mut meeting_count = 0;
                    for work in works.iter() {
                        if work.meets_target(target) {
                            meeting_count += 1;
                        }
                    }
                    std_black_box(meeting_count);
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark configuration performance under stress
fn bench_config_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_stress");
    
    // Test unit parsing performance with many values
    let test_values: Vec<String> = vec![
        // Standard values
        "1000", "1K", "1M", "1G", "1T",
        // Binary values  
        "1Ki", "1Mi", "1Gi", "1Ti", "1Pi",
        // Decimal values
        "1.5K", "2.7M", "3.14G", "42.0T",
        // Edge cases
        "0", "1", "999", "1000000000000",
    ];
    
    // Create many test cases
    let large_test_set: Vec<String> = (0..1000)
        .map(|i| {
            let base_values = ["K", "M", "G", "T", "Ki", "Mi", "Gi", "Ti"];
            let base = base_values[i % base_values.len()];
            format!("{}.{}{}", i % 1000, (i * 13) % 1000, base)
        })
        .collect();
    
    group.bench_function("parse_unit_prefix_stress", |b| {
        b.iter(|| {
            for value in &large_test_set {
                if let Ok(parsed) = units::parse_with_unit_prefix(value) {
                    std_black_box(parsed);
                }
            }
        });
    });
    
    group.bench_function("parse_hash_rate_stress", |b| {
        b.iter(|| {
            for value in &large_test_set {
                if let Ok(parsed) = units::parse_hash_rate(value) {
                    std_black_box(parsed);
                }
            }
        });
    });
    
    group.finish();
}

/// Real-world simulation benchmark
fn bench_production_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("production_simulation");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(20);
    
    // Simulate realistic mining pool scenario
    group.bench_function("mining_pool_simulation", |b| {
        b.iter_batched(
            || {
                // Setup: multiple miners, work distribution, target checking
                let miner_count = 10;
                let batch_size = 1000;
                let mut miners = Vec::new();
                for _ in 0..miner_count {
                    miners.push(VectorizedMiner::new(batch_size));
                }
                
                let target = Target::from_bytes({
                    let mut bytes = [0x00; 32];
                    bytes[0] = 0x00;
                    bytes[1] = 0x0F; // Medium difficulty
                    bytes
                });
                
                (miners, target)
            },
            |(mut miners, target)| {
                let mut total_hashes = 0u64;
                let mut solutions_found = 0u32;
                
                // Simulate mining round for each miner
                for (miner_id, miner) in miners.iter_mut().enumerate() {
                    let mut base_work = [0x42u8; 286];
                    base_work[0] = miner_id as u8; // Unique work per miner
                    
                    let start_nonce = miner_id as u64 * 100_000;
                    let hashes = miner.mine_batch(&base_work, start_nonce, 500);
                    
                    // Check for solutions
                    for (i, hash) in hashes.iter().enumerate() {
                        total_hashes += 1;
                        if target.meets_target(hash) {
                            solutions_found += 1;
                            // In real scenario, would submit to pool
                        }
                    }
                }
                
                std_black_box((total_hashes, solutions_found));
            },
            BatchSize::SmallInput,
        );
    });
    
    // Simulate Stratum server under load
    group.bench_function("stratum_server_simulation", |b| {
        b.iter(|| {
            // Simulate handling many concurrent mining connections
            let connection_count = 100;
            let jobs_per_connection = 10;
            
            let mut total_jobs = 0;
            let mut total_shares = 0;
            
            for conn_id in 0..connection_count {
                // Simulate job distribution
                for job_id in 0..jobs_per_connection {
                    let job_data = format!("job_{}_{}", conn_id, job_id);
                    total_jobs += 1;
                    
                    // Simulate share submissions (some connections more active)
                    let shares_for_job = if conn_id % 3 == 0 { 5 } else { 2 };
                    total_shares += shares_for_job;
                    
                    // Simulate share validation
                    for share_id in 0..shares_for_job {
                        let mut work = Work::from_bytes([0x33u8; 286]);
                        work.set_nonce(Nonce::new(share_id as u64 + job_id as u64 * 1000));
                        let hash = work.hash();
                        std_black_box(hash);
                    }
                }
            }
            
            std_black_box((total_jobs, total_shares));
        });
    });
    
    group.finish();
}

/// Benchmark memory usage and allocation patterns
fn bench_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_efficiency");
    group.measurement_time(Duration::from_secs(8));
    
    // Test memory allocation patterns
    group.bench_function("allocation_pattern_test", |b| {
        b.iter(|| {
            // Test the optimized allocation patterns
            let mut allocations = Vec::new();
            
            // Simulate mining client memory usage
            for i in 0..100 {
                // Work allocation
                let work = Work::from_bytes([i as u8; 286]);
                allocations.push(work);
                
                // Miner allocation  
                let miner = VectorizedMiner::new(64);
                std_black_box(miner);
                
                // Target allocation
                let target = Target::from_bytes([i as u8; 32]);
                std_black_box(target);
            }
            
            std_black_box(allocations.len());
        });
    });
    
    // Test buffer reuse efficiency
    group.bench_function("buffer_reuse_efficiency", |b| {
        b.iter_batched(
            || VectorizedMiner::new(1000),
            |mut miner| {
                // Reuse the same miner for multiple operations
                for i in 0..50 {
                    let mut base_work = [0x66u8; 286];
                    base_work[0] = i as u8;
                    let hashes = miner.mine_batch(&base_work, i as u64 * 1000, 100);
                    std_black_box(hashes);
                }
            },
            BatchSize::SmallInput,
        );
    });
    
    group.finish();
}

criterion_group!(
    comprehensive_benches,
    bench_vectorized_mining,
    bench_adaptive_hashing,
    bench_stress_mining,
    bench_http_pool_stress,
    bench_target_arithmetic_stress,
    bench_config_stress,
    bench_production_simulation,
    bench_memory_efficiency
);

criterion_main!(comprehensive_benches);