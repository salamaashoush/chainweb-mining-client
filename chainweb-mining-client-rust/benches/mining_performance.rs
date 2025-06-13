//! Performance benchmarks for mining operations

use chainweb_mining_client::core::{Nonce, Target, Work};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_hash_computation(c: &mut Criterion) {
    let work = Work::from_bytes([0x42u8; 286]);
    
    c.bench_function("blake2s_hash", |b| {
        b.iter(|| {
            black_box(work.hash());
        });
    });
}

fn bench_nonce_operations(c: &mut Criterion) {
    let mut work = Work::from_bytes([0x42u8; 286]);
    let nonce = Nonce::new(12345678);
    
    c.bench_function("set_nonce", |b| {
        b.iter(|| {
            work.set_nonce(black_box(nonce));
        });
    });
    
    c.bench_function("get_nonce", |b| {
        b.iter(|| {
            black_box(work.nonce());
        });
    });
}

fn bench_target_checking(c: &mut Criterion) {
    let work = Work::from_bytes([0x42u8; 286]);
    let target = Target::from_bytes([0xFF; 32]);
    
    c.bench_function("meets_target", |b| {
        b.iter(|| {
            black_box(work.meets_target(&target));
        });
    });
}

criterion_group!(
    benches,
    bench_hash_computation,
    bench_nonce_operations,
    bench_target_checking
);
criterion_main!(benches);