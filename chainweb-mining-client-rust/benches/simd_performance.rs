//! SIMD performance benchmarks

use chainweb_mining_client::core::{Target, Work, VectorizedMiner, SimdMiner, detect_simd_features};
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn benchmark_simd_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("blake2s_hashing");
    
    // Detect SIMD features
    let features = detect_simd_features();
    println!("Detected SIMD features: {}", features.description());
    
    // Create test work
    let work = Work::default();
    let work_bytes = *work.as_bytes();
    let target = Target::max_target(); // Easy target
    
    // Test different batch sizes
    let batch_sizes = [64, 128, 256, 512, 1024, 2048];
    
    for &batch_size in &batch_sizes {
        // Benchmark standard implementation
        let mut standard_miner = VectorizedMiner::new(batch_size);
        group.bench_with_input(
            BenchmarkId::new("standard", batch_size),
            &batch_size,
            |b, &size| {
                b.iter(|| {
                    standard_miner.mine_batch(&work_bytes, 0, size);
                });
            },
        );
        
        // Benchmark SIMD implementation
        let mut simd_miner = SimdMiner::new(batch_size);
        group.bench_with_input(
            BenchmarkId::new("simd", batch_size),
            &batch_size,
            |b, &size| {
                b.iter(|| {
                    simd_miner.mine_batch(&work_bytes, &target, 0, size);
                });
            },
        );
    }
    
    group.finish();
}

fn benchmark_single_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_hash");
    
    let work = Work::default();
    let work_bytes = *work.as_bytes();
    
    // Standard Blake2s
    group.bench_function("blake2_standard", |b| {
        use blake2::{Blake2s256, Digest};
        b.iter(|| {
            let mut hasher = Blake2s256::new();
            hasher.update(&work_bytes);
            let _hash = hasher.finalize();
        });
    });
    
    // SIMD Blake2s
    group.bench_function("blake2s_simd", |b| {
        use blake2s_simd::Params;
        let params = Params::new().hash_length(32);
        b.iter(|| {
            let _hash = params.hash(&work_bytes);
        });
    });
    
    group.finish();
}

fn benchmark_mining_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("mining_throughput");
    
    let work = Work::default();
    let work_bytes = *work.as_bytes();
    let target = Target::from_hex("00000000ffffffffffffffffffffffffffffffffffffffffffffffffffffffff").unwrap();
    
    // Measure hashes per second
    let duration = std::time::Duration::from_secs(5);
    
    // Standard implementation
    group.bench_function("standard_throughput", |b| {
        let mut miner = VectorizedMiner::new(1024);
        let mut nonce = 0u64;
        b.iter(|| {
            miner.mine_batch(&work_bytes, nonce, 1024);
            nonce += 1024;
        });
    });
    
    // SIMD implementation  
    group.bench_function("simd_throughput", |b| {
        let mut miner = SimdMiner::new(1024);
        let mut nonce = 0u64;
        b.iter(|| {
            miner.mine_batch(&work_bytes, &target, nonce, 1024);
            nonce += 1024;
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_simd_hashing,
    benchmark_single_hash,
    benchmark_mining_throughput
);
criterion_main!(benches);