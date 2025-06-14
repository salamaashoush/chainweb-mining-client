//! Stress tests for high-load scenarios and production readiness
//!
//! These tests push the system to its limits to ensure stability under extreme conditions.

use chainweb_mining_client::core::{Nonce, Target, Work, VectorizedMiner};
use chainweb_mining_client::protocol::http_pool::{HttpClientPool, HttpPoolConfig, ClientType};
use chainweb_mining_client::workers::cpu::{CpuWorker, CpuWorkerConfig};
use criterion::{
    Criterion, criterion_group, criterion_main, BenchmarkId, Throughput,
    black_box, BatchSize, measurement::WallTime, BenchmarkGroup
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

/// Stress test mining operations with extreme loads
fn stress_test_extreme_mining(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress_extreme_mining");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(10);
    
    // Test with very large batch sizes
    let extreme_batch_sizes = vec![10_000, 50_000, 100_000, 500_000];
    
    for &batch_size in &extreme_batch_sizes {
        group.throughput(Throughput::Elements(batch_size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("extreme_batch_mining", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.iter_batched(
                    || {
                        VectorizedMiner::new(batch_size)
                    },
                    |mut miner| {
                        let base_work = [0x88u8; 286];
                        let hashes = miner.mine_batch(&base_work, 0, batch_size);
                        black_box(hashes);
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }
    
    group.finish();
}

/// Stress test concurrent operations
fn stress_test_concurrency(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress_concurrency");
    group.measurement_time(Duration::from_secs(20));
    
    // Test many concurrent miners
    let thread_counts = vec![2, 4, 8, 16, 32, 64];
    
    for &thread_count in &thread_counts {
        group.bench_with_input(
            BenchmarkId::new("concurrent_miners", thread_count),
            &thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let hash_counter = Arc::new(AtomicU64::new(0));
                    let mut handles = Vec::new();
                    
                    for thread_id in 0..thread_count {
                        let counter = Arc::clone(&hash_counter);
                        let handle = thread::spawn(move || {
                            let mut miner = VectorizedMiner::new(1000);
                            let mut base_work = [0x99u8; 286];
                            base_work[0] = thread_id as u8;
                            
                            // Each thread mines for a fixed number of operations
                            for i in 0..100 {
                                let hashes = miner.mine_batch(
                                    &base_work, 
                                    thread_id as u64 * 100_000 + i * 1000, 
                                    500
                                );
                                counter.fetch_add(hashes.len() as u64, Ordering::Relaxed);
                            }
                        });
                        handles.push(handle);
                    }
                    
                    for handle in handles {
                        handle.join().unwrap();
                    }
                    
                    let total_hashes = hash_counter.load(Ordering::Relaxed);
                    black_box(total_hashes);
                });
            },
        );
    }
    
    group.finish();
}

/// Memory pressure stress test
fn stress_test_memory_pressure(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress_memory_pressure");
    group.measurement_time(Duration::from_secs(15));
    
    // Test allocation/deallocation patterns under pressure
    group.bench_function("allocation_deallocation_pressure", |b| {
        b.iter(|| {
            let mut miners = Vec::new();
            let mut works = Vec::new();
            let mut targets = Vec::new();
            
            // Rapid allocation phase
            for i in 0..1000 {
                miners.push(VectorizedMiner::new(100 + i % 900));
                
                let mut work_bytes = [0u8; 286];
                work_bytes[0] = (i % 256) as u8;
                work_bytes[1] = ((i / 256) % 256) as u8;
                works.push(Work::from_bytes(work_bytes));
                
                let mut target_bytes = [0u8; 32];
                target_bytes[0] = (i % 128) as u8;
                targets.push(Target::from_bytes(target_bytes));
            }
            
            // Usage phase
            for (i, miner) in miners.iter_mut().enumerate() {
                if i < works.len() {
                    let base_work = *works[i].as_bytes();
                    let hashes = miner.mine_batch(&base_work, i as u64 * 1000, 50);
                    black_box(hashes);
                }
            }
            
            // Cleanup happens automatically due to RAII
            black_box((miners.len(), works.len(), targets.len()));
        });
    });
    
    // Test sustained memory usage
    group.bench_function("sustained_memory_usage", |b| {
        b.iter_batched(
            || {
                // Setup sustained high memory usage
                let mut persistent_miners = Vec::new();
                for i in 0..100 {
                    persistent_miners.push(VectorizedMiner::new(1000 + i * 10));
                }
                persistent_miners
            },
            |mut miners| {
                // Sustained operations on large memory footprint
                for round in 0..50 {
                    for (i, miner) in miners.iter_mut().enumerate() {
                        let mut base_work = [0xAAu8; 286];
                        base_work[0] = (round + i) as u8;
                        let hashes = miner.mine_batch(&base_work, round as u64 * 10000 + i as u64 * 1000, 100);
                        black_box(hashes);
                    }
                }
            },
            BatchSize::LargeInput,
        );
    });
    
    group.finish();
}

/// HTTP pool stress testing
fn stress_test_http_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress_http_pool");
    group.measurement_time(Duration::from_secs(12));
    
    // Test pool under extreme concurrent access
    group.bench_function("extreme_concurrent_access", |b| {
        b.iter(|| {
            let mut config = HttpPoolConfig::default();
            config.enable_metrics = true;
            config.max_connections_per_host = 100;
            let pool = Arc::new(HttpClientPool::with_config(config));
            
            let mut handles = Vec::new();
            let access_count = Arc::new(AtomicU64::new(0));
            
            // Spawn many threads accessing the pool
            for thread_id in 0..50 {
                let pool_clone = Arc::clone(&pool);
                let counter = Arc::clone(&access_count);
                
                let handle = thread::spawn(move || {
                    for _ in 0..200 {
                        let client_type = match thread_id % 4 {
                            0 => ClientType::Mining,
                            1 => ClientType::Config,
                            2 => ClientType::General,
                            _ => ClientType::Insecure,
                        };
                        
                        if let Ok(_client) = pool_clone.get_client(client_type) {
                            counter.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                });
                handles.push(handle);
            }
            
            for handle in handles {
                handle.join().unwrap();
            }
            
            let total_accesses = access_count.load(Ordering::Relaxed);
            let stats = pool.get_stats();
            black_box((total_accesses, stats));
        });
    });
    
    // Test pool cache thrashing
    group.bench_function("cache_thrashing_test", |b| {
        b.iter(|| {
            let pool = HttpClientPool::new();
            
            // Rapidly create, clear, and recreate clients
            for cycle in 0..100 {
                // Create clients
                for _ in 0..10 {
                    let _mining = pool.get_client(ClientType::Mining).unwrap();
                    let _config = pool.get_client(ClientType::Config).unwrap();
                }
                
                // Clear cache every few cycles
                if cycle % 5 == 0 {
                    pool.clear_cache();
                }
            }
            
            let final_stats = pool.get_stats();
            black_box(final_stats);
        });
    });
    
    group.finish();
}

/// Target arithmetic stress testing
fn stress_test_target_arithmetic(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress_target_arithmetic");
    group.measurement_time(Duration::from_secs(10));
    
    // Test many target comparisons
    group.bench_function("massive_target_comparisons", |b| {
        b.iter_batched(
            || {
                // Setup many different targets and works
                let mut targets = Vec::new();
                let mut works = Vec::new();
                
                for i in 0..10000 {
                    let mut target_bytes = [0u8; 32];
                    target_bytes[0] = (i % 256) as u8;
                    target_bytes[1] = ((i / 256) % 256) as u8;
                    targets.push(Target::from_bytes(target_bytes));
                    
                    let mut work_bytes = [0u8; 286];
                    work_bytes[0] = ((i * 17) % 256) as u8;
                    work_bytes[1] = ((i * 23) % 256) as u8;
                    works.push(Work::from_bytes(work_bytes));
                }
                
                (targets, works)
            },
            |(targets, works)| {
                let mut meeting_count = 0;
                
                // Compare every work against every target
                for work in &works {
                    for target in &targets {
                        if work.meets_target(target) {
                            meeting_count += 1;
                        }
                    }
                }
                
                black_box(meeting_count);
            },
            BatchSize::LargeInput,
        );
    });
    
    group.finish();
}

/// Endurance testing - long-running operations
fn stress_test_endurance(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress_endurance");
    group.measurement_time(Duration::from_secs(60)); // Long measurement time
    group.sample_size(5); // Fewer samples due to long duration
    
    // Long-running mining simulation
    group.bench_function("long_running_mining", |b| {
        b.iter(|| {
            let start_time = Instant::now();
            let mut total_hashes = 0u64;
            let mut miner = VectorizedMiner::new(1000);
            
            // Mine for a sustained period
            while start_time.elapsed() < Duration::from_secs(10) {
                let base_work = [0xBBu8; 286];
                let hashes = miner.mine_batch(&base_work, total_hashes, 500);
                total_hashes += hashes.len() as u64;
                
                // Simulate some processing delay
                if total_hashes % 10000 == 0 {
                    thread::sleep(Duration::from_micros(100));
                }
            }
            
            black_box(total_hashes);
        });
    });
    
    // Sustained concurrent operations
    group.bench_function("sustained_concurrent_operations", |b| {
        b.iter(|| {
            let duration = Duration::from_secs(5);
            let start_time = Instant::now();
            let total_operations = Arc::new(AtomicU64::new(0));
            let mut handles = Vec::new();
            
            for thread_id in 0..8 {
                let counter = Arc::clone(&total_operations);
                let start = start_time;
                
                let handle = thread::spawn(move || {
                    let mut miner = VectorizedMiner::new(500);
                    let mut local_ops = 0u64;
                    
                    while start.elapsed() < duration {
                        let mut base_work = [0xCCu8; 286];
                        base_work[0] = thread_id as u8;
                        let hashes = miner.mine_batch(&base_work, local_ops, 100);
                        local_ops += hashes.len() as u64;
                        
                        if local_ops % 1000 == 0 {
                            counter.fetch_add(1000, Ordering::Relaxed);
                        }
                    }
                    
                    counter.fetch_add(local_ops % 1000, Ordering::Relaxed);
                });
                handles.push(handle);
            }
            
            for handle in handles {
                handle.join().unwrap();
            }
            
            let total_ops = total_operations.load(Ordering::Relaxed);
            black_box(total_ops);
        });
    });
    
    group.finish();
}

/// Resource exhaustion resistance testing
fn stress_test_resource_exhaustion(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress_resource_exhaustion");
    group.measurement_time(Duration::from_secs(8));
    
    // Test behavior when approaching system limits
    group.bench_function("approaching_memory_limits", |b| {
        b.iter(|| {
            let mut large_allocations = Vec::new();
            let mut allocation_count = 0;
            
            // Gradually increase allocation size until we hit reasonable limits
            let mut current_size = 1000;
            while current_size < 100_000 && allocation_count < 1000 {
                if let Ok(miner) = std::panic::catch_unwind(|| {
                    VectorizedMiner::new(current_size)
                }) {
                    large_allocations.push(miner);
                    allocation_count += 1;
                    current_size += 1000;
                } else {
                    break; // Hit allocation limit
                }
            }
            
            // Use some allocations to ensure they're not optimized away
            for (i, miner) in large_allocations.iter_mut().enumerate().take(10) {
                let base_work = [i as u8; 286];
                let hashes = miner.mine_batch(&base_work, i as u64 * 1000, 10);
                black_box(hashes);
            }
            
            black_box(allocation_count);
        });
    });
    
    group.finish();
}

criterion_group!(
    stress_tests,
    stress_test_extreme_mining,
    stress_test_concurrency,
    stress_test_memory_pressure,
    stress_test_http_pool,
    stress_test_target_arithmetic,
    stress_test_endurance,
    stress_test_resource_exhaustion
);

criterion_main!(stress_tests);