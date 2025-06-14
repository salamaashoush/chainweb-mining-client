//! Performance benchmarks for configuration operations

use chainweb_mining_client::config::{Config, FlatConfig, StratumDifficulty, WorkerConfig};
use chainweb_mining_client::utils::units;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::str::FromStr;

fn bench_config_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_parsing");

    // Sample YAML configuration
    let yaml_config = r#"
node:
  url: "api.chainweb.com"
  use_tls: true
  insecure: false
  timeout_secs: 30
  chain_id: 0

mining:
  account: "k:abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234"
  public_key: "abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234"
  update_interval_secs: 5

worker:
  type: "stratum"
  port: 1917
  host: "0.0.0.0"
  max_connections: 100
  difficulty: "block"
  rate_ms: 1000

logging:
  level: "info"
  format: "plain"
"#;

    // Sample JSON configuration
    let json_config = r#"{
  "node": "api.chainweb.com",
  "useTls": true,
  "insecure": false,
  "publicKey": "abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234",
  "account": "k:abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234",
  "threadCount": 4,
  "logLevel": "info",
  "worker": "stratum",
  "stratumPort": 1917,
  "stratumInterface": "0.0.0.0",
  "stratumDifficulty": "block",
  "stratumRate": 1000,
  "hashRate": 1000000,
  "defaultHTTPTimeout": 30000000
}"#;

    // Sample TOML configuration
    let toml_config = r#"
[node]
url = "api.chainweb.com"
use_tls = true
insecure = false
timeout_secs = 30
chain_id = 0

[mining]
account = "k:abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234"
public_key = "abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234"
update_interval_secs = 5

[worker]
type = "stratum"
port = 1917
host = "0.0.0.0"
max_connections = 100
difficulty = "block"
rate_ms = 1000

[logging]
level = "info"
format = "plain"
"#;

    group.bench_function("parse_yaml_config", |b| {
        b.iter(|| {
            black_box(serde_yaml::from_str::<Config>(yaml_config).unwrap());
        });
    });

    group.bench_function("parse_json_flat_config", |b| {
        b.iter(|| {
            black_box(serde_json::from_str::<FlatConfig>(json_config).unwrap());
        });
    });

    group.bench_function("parse_toml_config", |b| {
        b.iter(|| {
            black_box(toml::from_str::<Config>(toml_config).unwrap());
        });
    });

    group.finish();
}

fn bench_config_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_serialization");

    let config = Config::default();

    group.bench_function("serialize_yaml", |b| {
        b.iter(|| {
            black_box(serde_yaml::to_string(&config).unwrap());
        });
    });

    group.bench_function("serialize_json", |b| {
        b.iter(|| {
            black_box(serde_json::to_string(&config).unwrap());
        });
    });

    group.bench_function("serialize_toml", |b| {
        b.iter(|| {
            black_box(toml::to_string(&config).unwrap());
        });
    });

    group.finish();
}

fn bench_config_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_validation");

    let configs = vec![
        ("valid_cpu", {
            let mut config = Config::default();
            config.worker = WorkerConfig::Cpu {
                threads: 4,
                batch_size: 100_000,
            };
            config
        }),
        ("valid_stratum", {
            let mut config = Config::default();
            config.worker = WorkerConfig::Stratum {
                port: 1917,
                host: "0.0.0.0".to_string(),
                max_connections: 100,
                difficulty: StratumDifficulty::Block,
                rate_ms: 1000,
            };
            config
        }),
        ("valid_simulation", {
            let mut config = Config::default();
            config.worker = WorkerConfig::Simulation {
                hash_rate: 1_000_000.0,
            };
            config
        }),
    ];

    for (name, config) in &configs {
        group.bench_with_input(
            BenchmarkId::new("validate_config", name),
            config,
            |b, config| {
                b.iter(|| {
                    black_box(config.validate().is_ok());
                });
            },
        );
    }

    group.finish();
}

fn bench_unit_parsing_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("unit_parsing_performance");
    group.throughput(Throughput::Elements(100));

    let test_cases = vec![
        "1000", "1K", "1.5K", "2K", "10K", "100K", "1M", "2.5M", "10M", "100M", "1000M", "1G",
        "1.5G", "2G", "10G", "1T", "2T", "1Ki", "1Mi", "1Gi", "1Ti", "512Ki", "256Mi", "4Gi",
        "1024Gi",
    ];

    group.bench_function("parse_various_units", |b| {
        b.iter(|| {
            for case in &test_cases {
                black_box(units::parse_with_unit(case).unwrap());
            }
        });
    });

    group.bench_function("parse_hash_rates", |b| {
        b.iter(|| {
            for case in &test_cases {
                black_box(units::parse_hash_rate(case).unwrap());
            }
        });
    });

    group.bench_function("parse_memory_sizes", |b| {
        b.iter(|| {
            for case in &test_cases {
                if let Ok(val) = units::parse_with_unit(case) {
                    if val <= u64::MAX as f64 {
                        black_box(val as u64);
                    }
                }
            }
        });
    });

    group.finish();
}

fn bench_stratum_difficulty_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("stratum_difficulty_parsing");

    let difficulty_strings = vec![
        "block", "1", "8", "16", "32", "64", "128", "256", "0", "255",
    ];

    group.bench_function("parse_stratum_difficulties", |b| {
        b.iter(|| {
            for difficulty_str in &difficulty_strings {
                black_box(StratumDifficulty::from_str(difficulty_str).unwrap());
            }
        });
    });

    group.finish();
}

fn bench_worker_config_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("worker_config_creation");

    group.bench_function("create_cpu_worker", |b| {
        b.iter(|| {
            black_box(WorkerConfig::Cpu {
                threads: 4,
                batch_size: 100_000,
            });
        });
    });

    group.bench_function("create_stratum_worker", |b| {
        b.iter(|| {
            black_box(WorkerConfig::Stratum {
                port: 1917,
                host: "0.0.0.0".to_string(),
                max_connections: 100,
                difficulty: StratumDifficulty::Block,
                rate_ms: 1000,
            });
        });
    });

    group.bench_function("create_external_worker", |b| {
        b.iter(|| {
            black_box(WorkerConfig::External {
                command: "/usr/bin/external-miner".to_string(),
                args: vec!["--threads".to_string(), "4".to_string()],
                env: vec![("GPU_FORCE_64BIT_PTR".to_string(), "1".to_string())],
                timeout_secs: 60,
            });
        });
    });

    group.finish();
}

fn bench_config_merging(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_merging");

    let base_config = Config::default();
    let override_configs = vec![
        {
            let mut config = Config::default();
            config.node.url = "different-node.com".to_string();
            config
        },
        {
            let mut config = Config::default();
            config.worker = WorkerConfig::Cpu {
                threads: 8,
                batch_size: 50_000,
            };
            config
        },
        {
            let mut config = Config::default();
            config.logging.level = "debug".to_string();
            config
        },
    ];

    group.bench_function("merge_configs", |b| {
        b.iter(|| {
            let mut result = base_config.clone();
            for override_config in &override_configs {
                // Simulate merging (simplified)
                result = override_config.clone();
            }
            black_box(result);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_config_parsing,
    bench_config_serialization,
    bench_config_validation,
    bench_unit_parsing_performance,
    bench_stratum_difficulty_parsing,
    bench_worker_config_creation,
    bench_config_merging
);
criterion_main!(benches);
