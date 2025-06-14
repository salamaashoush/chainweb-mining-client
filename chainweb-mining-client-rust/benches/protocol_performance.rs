//! Performance benchmarks for protocol operations

use base64::Engine;
use chainweb_mining_client::core::{ChainId, Target, Work};
use chainweb_mining_client::error::Error;
use chainweb_mining_client::protocol::retry::should_retry;
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::time::Duration;

fn bench_json_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_serialization");

    // Sample chainweb work request/response data
    let work_request = serde_json::json!({
        "account": "k:abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234",
        "predicate": "keys-all",
        "public-keys": ["abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234"]
    });

    let work_response = serde_json::json!({
        "target": "0000ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        "header": "000000000123456789abcdef000000000123456789abcdef000000000123456789abcdef000000000123456789abcdef000000000123456789abcdef000000000123456789abcdef000000000123456789abcdef000000000123456789abcdef000000000123456789abcdef000000000123456789abcdef000000000123456789abcdef000000000123456789abcdef000000000123456789abcdef000000000123456789abcdef000000000123456789abcdef000000000123456789abcdef000000000123456789abcdef000000000123456789abcdef000000000123456789abcdef000000000123456789abcdef"
    });

    group.bench_function("serialize_work_request", |b| {
        b.iter(|| {
            black_box(serde_json::to_string(&work_request).unwrap());
        });
    });

    group.bench_function("deserialize_work_response", |b| {
        let json_str = serde_json::to_string(&work_response).unwrap();
        b.iter(|| {
            black_box(serde_json::from_str::<serde_json::Value>(&json_str).unwrap());
        });
    });

    group.finish();
}

fn bench_binary_protocol(c: &mut Criterion) {
    let mut group = c.benchmark_group("binary_protocol");

    let chain_id = ChainId::new(5);
    let target = Target::from_bytes([
        0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00,
        0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF,
        0x00, 0xFF,
    ]);
    let work = Work::from_bytes({
        let mut data = [0u8; 286];
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = ((i * 7) % 256) as u8;
        }
        data
    });

    group.bench_function("encode_chain_id", |b| {
        b.iter(|| {
            black_box((chain_id.value() as u32).to_le_bytes());
        });
    });

    group.bench_function("encode_target", |b| {
        b.iter(|| {
            black_box(target.as_bytes());
        });
    });

    group.bench_function("encode_work", |b| {
        b.iter(|| {
            black_box(work.as_bytes());
        });
    });

    group.bench_function("decode_work", |b| {
        let bytes = work.as_bytes();
        b.iter(|| {
            black_box(Work::from_bytes(*bytes));
        });
    });

    group.finish();
}

fn bench_retry_logic(c: &mut Criterion) {
    let mut group = c.benchmark_group("retry_logic");

    let errors = vec![
        Error::network("Connection timeout"),
        Error::network("server error 503"),
        Error::protocol("Gateway Timeout"),
        Error::config("Invalid key"),
        Error::timeout("Request timeout"),
        Error::Io(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "Connection refused",
        )),
    ];

    group.bench_function("should_retry_decisions", |b| {
        b.iter(|| {
            for error in &errors {
                black_box(should_retry(error));
            }
        });
    });

    group.finish();
}

fn bench_http_client_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("http_client");

    group.bench_function("create_reqwest_client", |b| {
        b.iter(|| {
            black_box(reqwest::Client::new());
        });
    });

    group.bench_function("create_reqwest_client_with_timeout", |b| {
        b.iter(|| {
            black_box(
                reqwest::Client::builder()
                    .timeout(Duration::from_secs(30))
                    .build()
                    .unwrap(),
            );
        });
    });

    group.finish();
}

fn bench_hex_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("hex_operations");

    let data = vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
    let hex_string = "123456789ABCDEF0";

    group.bench_function("encode_hex", |b| {
        b.iter(|| {
            black_box(hex::encode(&data));
        });
    });

    group.bench_function("decode_hex", |b| {
        b.iter(|| {
            black_box(hex::decode(hex_string).unwrap());
        });
    });

    // Benchmark encoding/decoding larger data (like work headers)
    let large_data: Vec<u8> = (0..286).map(|i| (i % 256) as u8).collect();
    let large_hex = hex::encode(&large_data);

    group.bench_function("encode_hex_286_bytes", |b| {
        b.iter(|| {
            black_box(hex::encode(&large_data));
        });
    });

    group.bench_function("decode_hex_286_bytes", |b| {
        b.iter(|| {
            black_box(hex::decode(&large_hex).unwrap());
        });
    });

    group.finish();
}

fn bench_base64_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("base64_operations");

    let data = vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
    let base64_string = "EjRWeJq83vA=";

    group.bench_function("encode_base64", |b| {
        b.iter(|| {
            black_box(base64::engine::general_purpose::STANDARD.encode(&data));
        });
    });

    group.bench_function("decode_base64", |b| {
        b.iter(|| {
            black_box(
                base64::engine::general_purpose::STANDARD
                    .decode(base64_string)
                    .unwrap(),
            );
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_json_serialization,
    bench_binary_protocol,
    bench_retry_logic,
    bench_http_client_creation,
    bench_hex_operations,
    bench_base64_operations
);
criterion_main!(benches);
