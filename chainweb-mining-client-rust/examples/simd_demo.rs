//! SIMD performance demonstration

use chainweb_mining_client::core::{Work, Target, VectorizedMiner, SimdMiner, detect_simd_features};
use std::time::Instant;

fn main() {
    println!("SIMD Performance Demo");
    println!("====================\n");
    
    // Detect CPU features
    let features = detect_simd_features();
    println!("CPU Features: {}", features.description());
    println!();
    
    // Create test data
    let work = Work::default();
    let work_bytes = *work.as_bytes();
    let target = Target::from_hex("00000000ffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
        .expect("Valid target");
    
    const BATCH_SIZE: usize = 1024;
    const ITERATIONS: usize = 10000;
    
    // Test 1: Standard implementation
    println!("Test 1: Standard Blake2s implementation");
    let mut standard_miner = VectorizedMiner::new(BATCH_SIZE);
    let start = Instant::now();
    let mut nonce = 0u64;
    for _ in 0..ITERATIONS {
        standard_miner.mine_batch(&work_bytes, nonce, BATCH_SIZE);
        nonce += BATCH_SIZE as u64;
    }
    let standard_time = start.elapsed();
    let standard_hashes = ITERATIONS * BATCH_SIZE;
    let standard_rate = standard_hashes as f64 / standard_time.as_secs_f64();
    println!("Time: {:?}", standard_time);
    println!("Hashes: {}", standard_hashes);
    println!("Hash rate: {:.2} MH/s", standard_rate / 1_000_000.0);
    
    // Test 2: SIMD implementation
    println!("\nTest 2: SIMD-optimized Blake2s implementation");
    let mut simd_miner = SimdMiner::new(BATCH_SIZE);
    let start = Instant::now();
    let mut nonce = 0u64;
    for _ in 0..ITERATIONS {
        simd_miner.mine_batch(&work_bytes, &target, nonce, BATCH_SIZE);
        nonce += BATCH_SIZE as u64;
    }
    let simd_time = start.elapsed();
    let simd_hashes = ITERATIONS * BATCH_SIZE;
    let simd_rate = simd_hashes as f64 / simd_time.as_secs_f64();
    println!("Time: {:?}", simd_time);
    println!("Hashes: {}", simd_hashes);
    println!("Hash rate: {:.2} MH/s", simd_rate / 1_000_000.0);
    
    // Calculate speedup
    let speedup = simd_rate / standard_rate;
    
    println!("\nPerformance Summary:");
    println!("====================");
    println!("SIMD implementation is {:.2}x faster", speedup);
    println!("Standard: {:.2} MH/s", standard_rate / 1_000_000.0);
    println!("SIMD:     {:.2} MH/s", simd_rate / 1_000_000.0);
    
    // Additional benefits
    println!("\nAdditional SIMD Benefits:");
    println!("- Automatic CPU feature detection");
    println!("- Optimized for AVX2, SSE4.1, SSSE3, and ARM NEON");
    println!("- Better CPU cache utilization");
    println!("- Lower power consumption per hash");
}