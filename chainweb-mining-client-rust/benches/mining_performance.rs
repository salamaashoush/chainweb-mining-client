//! Performance benchmarks for mining operations

use chainweb_mining_client::core::{ChainId, Nonce, Target, Work};
use chainweb_mining_client::utils::units;
use criterion::{Criterion, criterion_group, criterion_main, BenchmarkId, Throughput};
use rand::Rng;
use std::hint::black_box;

fn bench_hash_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_computation");
    
    // Test with different work patterns
    let patterns = vec![
        ("zeros", [0u8; 286]),
        ("ones", [0xFFu8; 286]),
        ("pattern", {
            let mut data = [0u8; 286];
            for (i, byte) in data.iter_mut().enumerate() {
                *byte = (i % 256) as u8;
            }
            data
        }),
        ("random", {
            let mut rng = rand::rng();
            let mut data = [0u8; 286];
            rng.fill(&mut data);
            data
        })
    ];
    
    for (name, data) in patterns {
        let work = Work::from_bytes(data);
        group.bench_with_input(BenchmarkId::new("blake2s_hash", name), &work, |b, work| {
            b.iter(|| {
                black_box(work.hash());
            });
        });
    }
    
    group.finish();
}

fn bench_nonce_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("nonce_operations");
    
    let mut work = Work::from_bytes([0x42u8; 286]);
    let nonces = vec![
        Nonce::new(0),
        Nonce::new(12345678),
        Nonce::new(u64::MAX),
        Nonce::new(0x0123456789ABCDEF),
    ];

    group.bench_function("set_nonce", |b| {
        b.iter(|| {
            for nonce in &nonces {
                work.set_nonce(black_box(*nonce));
            }
        });
    });

    group.bench_function("get_nonce", |b| {
        b.iter(|| {
            black_box(work.nonce());
        });
    });
    
    group.finish();
}

fn bench_target_checking(c: &mut Criterion) {
    let mut group = c.benchmark_group("target_checking");
    
    let work_easy = Work::from_bytes([0x00u8; 286]); // Should meet most targets
    let work_hard = Work::from_bytes([0xFFu8; 286]); // Should meet few targets
    
    let targets = vec![
        ("very_easy", Target::from_bytes([0xFF; 32])),
        ("easy", Target::from_bytes({
            let mut bytes = [0u8; 32];
            bytes[0] = 0x0F;
            bytes
        })),
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
        group.bench_with_input(BenchmarkId::new("meets_target_easy", name), &(work_easy.clone(), target), |b, (work, target)| {
            b.iter(|| {
                black_box(work.meets_target(target));
            });
        });
        
        group.bench_with_input(BenchmarkId::new("meets_target_hard", name), &(work_hard.clone(), target), |b, (work, target)| {
            b.iter(|| {
                black_box(work.meets_target(target));
            });
        });
    }
    
    group.finish();
}

fn bench_mining_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("mining_simulation");
    group.throughput(Throughput::Elements(1000));
    
    let mut work = Work::from_bytes([0x42u8; 286]);
    let target = Target::from_bytes({
        let mut bytes = [0u8; 32];
        bytes[0] = 0x00;
        bytes[1] = 0x0F;
        bytes
    });
    
    group.bench_function("mine_1000_nonces", |b| {
        b.iter(|| {
            let mut nonce_val = 0u64;
            for _ in 0..1000 {
                work.set_nonce(Nonce::new(nonce_val));
                let _hash = work.hash();
                let _meets = work.meets_target(&target);
                nonce_val = nonce_val.wrapping_add(1);
            }
            black_box(nonce_val);
        });
    });
    
    group.finish();
}

fn bench_chain_id_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("chain_id_operations");
    
    let chain_ids: Vec<ChainId> = (0..20).map(ChainId::new).collect();
    
    group.bench_function("create_chain_ids", |b| {
        b.iter(|| {
            for i in 0..20 {
                black_box(ChainId::new(i));
            }
        });
    });
    
    group.bench_function("get_chain_id_values", |b| {
        b.iter(|| {
            for chain_id in &chain_ids {
                black_box(chain_id.value());
            }
        });
    });
    
    group.finish();
}

fn bench_unit_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("unit_parsing");
    
    let test_cases = vec![
        "1000",
        "1K",
        "2.5M",
        "1Gi",
        "500Mi",
        "1.5T",
        "100Ki",
    ];
    
    group.bench_function("parse_with_unit_prefix", |b| {
        b.iter(|| {
            for case in &test_cases {
                black_box(units::parse_with_unit_prefix(case).unwrap());
            }
        });
    });
    
    group.bench_function("parse_hash_rate", |b| {
        b.iter(|| {
            for case in &test_cases {
                black_box(units::parse_hash_rate(case).unwrap());
            }
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_hash_computation,
    bench_nonce_operations,
    bench_target_checking,
    bench_mining_simulation,
    bench_chain_id_operations,
    bench_unit_parsing
);
criterion_main!(benches);