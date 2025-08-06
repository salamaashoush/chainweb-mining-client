//! Memory performance benchmarks

use chainweb_mining_client::core::Work;
use chainweb_mining_client::utils::memory::{PooledWork, WorkPool};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_work_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("work_allocation");
    
    // Benchmark direct allocation
    group.bench_function("direct_allocation", |b| {
        b.iter(|| {
            let work = Box::new(Work::default());
            black_box(work);
        });
    });
    
    // Benchmark pooled allocation
    let pool = WorkPool::new(1024);
    group.bench_function("pooled_allocation", |b| {
        b.iter(|| {
            let work = pool.get();
            black_box(&work);
            pool.put(work);
        });
    });
    
    // Benchmark pooled work with guard
    group.bench_function("pooled_work_guard", |b| {
        b.iter(|| {
            let work = PooledWork::get();
            black_box(&*work);
            // Automatically returned to pool on drop
        });
    });
    
    group.finish();
}

fn benchmark_bulk_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulk_operations");
    
    // Benchmark allocating many work objects directly
    group.bench_function("bulk_direct_allocation", |b| {
        b.iter(|| {
            let mut works = Vec::with_capacity(100);
            for _ in 0..100 {
                works.push(Box::new(Work::default()));
            }
            black_box(works);
        });
    });
    
    // Benchmark allocating many work objects from pool
    let pool = WorkPool::new(1024);
    group.bench_function("bulk_pooled_allocation", |b| {
        b.iter(|| {
            let mut works = Vec::with_capacity(100);
            for _ in 0..100 {
                works.push(pool.get());
            }
            for work in works {
                pool.put(work);
            }
        });
    });
    
    group.finish();
}

criterion_group!(benches, benchmark_work_allocation, benchmark_bulk_operations);
criterion_main!(benches);