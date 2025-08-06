//! Demonstration of memory optimizations

use chainweb_mining_client::core::Work;
use chainweb_mining_client::utils::memory::{WorkPool, PooledWork};
use std::time::Instant;

fn main() {
    println!("Memory Optimization Demo");
    println!("========================\n");
    
    const ITERATIONS: usize = 1_000_000;
    
    // Test 1: Direct allocation
    println!("Test 1: Direct allocation");
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _work = Box::new(Work::default());
        // Work is dropped here
    }
    let direct_time = start.elapsed();
    println!("Time for {} direct allocations: {:?}", ITERATIONS, direct_time);
    
    // Test 2: Pooled allocation
    println!("\nTest 2: Pooled allocation");
    let pool = WorkPool::new(1024);
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let work = pool.get();
        pool.put(work);
    }
    let pooled_time = start.elapsed();
    println!("Time for {} pooled allocations: {:?}", ITERATIONS, pooled_time);
    
    // Test 3: Pooled with guard
    println!("\nTest 3: Pooled allocation with guard");
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _work = PooledWork::get();
        // Work is automatically returned to pool on drop
    }
    let guard_time = start.elapsed();
    println!("Time for {} guarded allocations: {:?}", ITERATIONS, guard_time);
    
    // Calculate improvements
    let pool_speedup = direct_time.as_secs_f64() / pooled_time.as_secs_f64();
    let guard_speedup = direct_time.as_secs_f64() / guard_time.as_secs_f64();
    
    println!("\nPerformance Summary:");
    println!("====================");
    println!("Pooled allocation is {:.2}x faster than direct allocation", pool_speedup);
    println!("Guarded pooled allocation is {:.2}x faster than direct allocation", guard_speedup);
    
    // Memory usage comparison
    println!("\nMemory Usage Benefits:");
    println!("- Object pooling reduces heap allocations");
    println!("- Reuses memory, reducing fragmentation");
    println!("- Better cache locality for frequently used objects");
    println!("- Jemalloc provides better multi-threaded performance");
}