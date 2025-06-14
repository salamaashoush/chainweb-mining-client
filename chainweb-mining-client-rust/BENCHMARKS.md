# Performance Benchmarks

This document describes the comprehensive benchmark suite for `chainweb-mining-client-rust`, designed to measure performance across all critical components.

## Overview

The benchmark suite consists of four main categories:

1. **Mining Performance** - Core mining operations
2. **Protocol Performance** - Network protocol operations  
3. **Stratum Performance** - Stratum protocol operations
4. **Configuration Performance** - Config parsing and validation

## Benchmark Categories

### 1. Mining Performance (`mining_performance`)

Tests the core mining operations that are critical for hash rate and mining efficiency:

#### Hash Computation
- **blake2s_hash**: Blake2s-256 hashing with different data patterns
- Tests various input patterns: zeros, ones, patterns, random data
- **Key Metric**: Nanoseconds per hash operation

#### Nonce Operations  
- **set_nonce**: Setting nonce values in work headers
- **get_nonce**: Reading nonce values from work headers
- Tests various nonce values: 0, typical values, max values
- **Key Metric**: Nanoseconds per operation

#### Target Checking
- **meets_target**: Checking if hash meets mining difficulty target
- Tests with different difficulty levels: very easy to hard
- Tests with different hash patterns: easy vs hard to meet targets
- **Key Metric**: Nanoseconds per target check

#### Mining Simulation
- **mine_1000_nonces**: Simulates mining 1000 sequential nonces
- Measures complete mining loop: set nonce → hash → check target
- **Key Metric**: Elements/second throughput

#### Chain ID Operations
- **create_chain_ids**: Creating ChainId objects (0-19)
- **get_chain_id_values**: Reading values from ChainId objects
- **Key Metric**: Nanoseconds per operation

#### Unit Parsing
- **parse_with_unit_prefix**: Parsing values with SI/binary unit prefixes
- **parse_hash_rate**: Hash rate parsing with units
- Tests: K, M, G, Ki, Mi, Gi prefixes
- **Key Metric**: Nanoseconds per parse operation

### 2. Protocol Performance (`protocol_performance`)

Tests network protocol operations and data serialization:

#### JSON Serialization
- **serialize_work_request**: Serializing mining work requests
- **deserialize_work_response**: Deserializing chainweb responses
- **Key Metric**: Nanoseconds per operation

#### Binary Protocol
- **encode_chain_id**: Encoding chain IDs to 4-byte little-endian
- **encode_target**: Encoding 32-byte targets
- **encode_work**: Encoding 286-byte work headers
- **decode_work**: Decoding work headers from bytes
- **Key Metric**: Nanoseconds per encode/decode operation

#### Retry Logic
- **should_retry_decisions**: HTTP retry decision logic
- Tests various error types: network, protocol, config, timeout
- **Key Metric**: Nanoseconds per decision

#### HTTP Client Creation
- **create_reqwest_client**: Creating HTTP clients
- **create_reqwest_client_with_timeout**: Creating configured clients
- **Key Metric**: Nanoseconds per client creation

#### Hex Operations
- **encode_hex**: Hex encoding of binary data
- **decode_hex**: Hex decoding to binary data
- Tests small (8 bytes) and large (286 bytes) data
- **Key Metric**: Nanoseconds per operation

#### Base64 Operations
- **encode_base64**: Base64 encoding
- **decode_base64**: Base64 decoding
- **Key Metric**: Nanoseconds per operation

### 3. Stratum Performance (`stratum_performance`)

Tests Stratum mining protocol operations:

#### Stratum Message Parsing
- **parse_json**: Parsing Stratum JSON-RPC messages
- Tests: subscribe, authorize, submit, notify messages
- **Key Metric**: Nanoseconds per message parse

#### Stratum Message Generation
- **generate_job_notify**: Creating mining job notifications
- **generate_set_difficulty**: Creating difficulty adjustment messages
- **Key Metric**: Nanoseconds per message generation

#### Nonce Splitting
- **split_8_byte_nonce**: Splitting 64-bit nonces into Nonce1/Nonce2
- **combine_split_nonces**: Combining split nonces back
- **partition_nonce_space**: Partitioning nonce space for multiple miners
- **Key Metric**: Nanoseconds per operation

#### Difficulty Adjustment
- **calculate_target_from_difficulty**: Converting difficulty to target
- Tests various difficulty levels: 1, 16, 256, 4096, 65536
- **Key Metric**: Nanoseconds per calculation

#### Job Management
- **job_storage_operations**: Job storage, lookup, and cleanup
- Simulates 100 job additions, 50 lookups, 25 deletions
- **Key Metric**: Nanoseconds per complete operation set

#### Share Validation
- **validate_share**: Validating mining shares
- Tests different difficulty targets: easy, medium, hard
- Measures hash computation + target checking
- **Key Metric**: Nanoseconds per share validation

### 4. Configuration Performance (`config_performance`)

Tests configuration parsing and validation:

#### Config Parsing
- **parse_yaml_config**: Parsing YAML configuration files
- **parse_json_flat_config**: Parsing JSON flat config (Haskell-compatible)
- **parse_toml_config**: Parsing TOML configuration files
- **Key Metric**: Nanoseconds per config parse

#### Config Serialization
- **serialize_yaml**: Serializing config to YAML
- **serialize_json**: Serializing config to JSON
- **serialize_toml**: Serializing config to TOML
- **Key Metric**: Nanoseconds per serialization

#### Config Validation
- **validate_config**: Validating configuration objects
- Tests different worker types: CPU, Stratum, Simulation
- **Key Metric**: Nanoseconds per validation

#### Unit Parsing Performance
- **parse_various_units**: Parsing unit values with prefixes
- **parse_hash_rates**: Hash rate specific parsing
- **parse_memory_sizes**: Memory size parsing
- Tests 24 different unit combinations
- **Key Metric**: Elements/second throughput

#### Stratum Difficulty Parsing
- **parse_stratum_difficulties**: Parsing difficulty settings
- Tests: "block", numeric values 0-256
- **Key Metric**: Nanoseconds per parse

#### Worker Config Creation
- **create_cpu_worker**: Creating CPU worker configs
- **create_stratum_worker**: Creating Stratum worker configs  
- **create_external_worker**: Creating external worker configs
- **Key Metric**: Nanoseconds per config creation

#### Config Merging
- **merge_configs**: Merging configuration objects
- Tests merging base config with multiple overrides
- **Key Metric**: Nanoseconds per merge operation

## Running Benchmarks

### Quick Start

```bash
# Run all benchmarks with default settings
./scripts/bench.sh

# Quick run with fewer samples (faster)
./scripts/bench.sh --quick

# Save results as baseline for comparison
./scripts/bench.sh --save-baseline main

# Compare against saved baseline
./scripts/bench.sh --compare-baseline main

# Generate HTML reports
./scripts/bench.sh --output-dir ./reports
```

### Individual Benchmarks

```bash
# Run specific benchmark category
cargo bench --features=bench --bench mining_performance

# Run specific benchmark test
cargo bench --features=bench --bench mining_performance hash_computation

# Run with custom criterion options
cargo bench --features=bench --bench mining_performance -- --sample-size 100
```

## Performance Targets

Based on mining requirements, here are suggested performance targets:

### Critical Path (Mining)
- **blake2s_hash**: < 1,000 ns (target: ~270 ns achieved)
- **meets_target**: < 500 ns
- **set_nonce**: < 100 ns
- **mine_1000_nonces**: > 1,000,000 elements/second

### Protocol Operations
- **JSON serialization**: < 1,000 ns
- **Binary encoding**: < 500 ns
- **Retry decisions**: < 100 ns

### Configuration
- **Config parsing**: < 100,000 ns (100 μs)
- **Unit parsing**: > 100,000 elements/second
- **Config validation**: < 10,000 ns

## Interpreting Results

### Key Metrics

1. **Time per Operation**: Lower is better
   - Nanoseconds (ns) for micro-operations
   - Microseconds (μs) for complex operations

2. **Throughput**: Higher is better
   - Elements/second for batch operations
   - Operations/second for continuous operations

3. **Consistency**: Lower variance is better
   - Standard deviation in timing
   - Outlier detection and removal

### Comparison Guidelines

- **Green**: Performance improvement
- **Red**: Performance regression
- **Yellow**: No significant change

Regressions > 5% should be investigated.
Improvements > 10% should be verified and documented.

## CI Integration

Benchmarks can be integrated into CI/CD pipelines:

```bash
# Fast CI benchmarks (quick mode)
./scripts/bench.sh --quick --save-baseline ci-baseline

# Performance regression detection
./scripts/bench.sh --compare-baseline ci-baseline
```

## Hardware Considerations

Benchmark results are highly dependent on hardware:

- **CPU**: Hash computation performance scales with CPU performance
- **Memory**: Large datasets may be memory bandwidth limited  
- **Storage**: Config parsing may be I/O limited for large files

For consistent results:
- Run on dedicated hardware
- Disable CPU frequency scaling
- Close unnecessary applications
- Use multiple samples for averaging

## Benchmark Data

All benchmark results are stored in `target/criterion/` with:
- Raw timing data
- Statistical analysis
- HTML reports with graphs
- Historical comparisons

Results can be archived for long-term performance tracking.