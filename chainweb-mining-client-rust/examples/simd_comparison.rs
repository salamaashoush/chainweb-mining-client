//! Fair SIMD performance comparison

use blake2::{Blake2s256, Digest};
use blake2s_simd::Params;
use std::time::Instant;

fn main() {
    println!("Blake2s Performance Comparison");
    println!("==============================\n");
    
    // Test data
    let data = vec![0u8; 286]; // Work size
    const ITERATIONS: usize = 1_000_000;
    
    // Test 1: Standard blake2 crate
    println!("Test 1: Standard blake2 crate");
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let mut hasher = Blake2s256::new();
        hasher.update(&data);
        let _hash = hasher.finalize();
    }
    let standard_time = start.elapsed();
    let standard_rate = ITERATIONS as f64 / standard_time.as_secs_f64();
    println!("Time: {:?}", standard_time);
    println!("Hash rate: {:.2} MH/s", standard_rate / 1_000_000.0);
    
    // Test 2: blake2s_simd crate
    println!("\nTest 2: blake2s_simd crate");
    let params = Params::new().hash_length(32).to_owned();
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _hash = params.hash(&data);
    }
    let simd_time = start.elapsed();
    let simd_rate = ITERATIONS as f64 / simd_time.as_secs_f64();
    println!("Time: {:?}", simd_time);
    println!("Hash rate: {:.2} MH/s", simd_rate / 1_000_000.0);
    
    // Test 3: blake2s_simd with state reuse
    println!("\nTest 3: blake2s_simd with state reuse");
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let mut state = params.to_state();
        state.update(&data);
        let _hash = state.finalize();
    }
    let simd_reuse_time = start.elapsed();
    let simd_reuse_rate = ITERATIONS as f64 / simd_reuse_time.as_secs_f64();
    println!("Time: {:?}", simd_reuse_time);
    println!("Hash rate: {:.2} MH/s", simd_reuse_rate / 1_000_000.0);
    
    // Calculate speedup
    let speedup1 = simd_rate / standard_rate;
    let speedup2 = simd_reuse_rate / standard_rate;
    
    println!("\nPerformance Summary:");
    println!("====================");
    println!("blake2s_simd is {:.2}x faster", speedup1);
    println!("blake2s_simd (with state reuse) is {:.2}x faster", speedup2);
    
    // CPU features
    println!("\nCPU SIMD Features:");
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("avx2") {
            println!("- AVX2: Yes");
        }
        if is_x86_feature_detected!("sse4.1") {
            println!("- SSE4.1: Yes");
        }
    }
}