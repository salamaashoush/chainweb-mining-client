# E2E Tests and Benchmarks

This directory contains comprehensive end-to-end tests and performance benchmarks for comparing the Haskell and Rust implementations of the chainweb mining client.

## Prerequisites

- Docker and Docker Compose
- jq (for JSON processing)
- bash
- At least 8GB of available RAM
- Network access to pull Docker images

## Building Docker Images

### Local Build (Recommended for Testing)

To build the Rust implementation Docker image locally without pushing:

```bash
# From the rust project root
cd ../..
./build-local.sh

# Or use the full build script with --local flag
./build-docker.sh latest scratch --local
```

This builds only for your current platform and doesn't push to Docker Hub.

### Publishing Images

To build and push multi-architecture images (requires Docker Hub access):

```bash
./build-docker.sh latest scratch
```

## Quick Start

### Run All E2E Tests

```bash
./run-e2e-tests.sh
```

This will test all worker types with both implementations and generate a comparison report.

### Run Performance Benchmarks

```bash
./run-benchmarks.sh
```

This will run performance benchmarks comparing CPU and Stratum worker performance.

## E2E Tests

The E2E test suite validates that both Haskell and Rust implementations work correctly with actual Chainweb nodes.

### Worker Types Tested

1. **CPU Worker** - Multi-threaded CPU mining with Blake2s-256
2. **Stratum Server** - Stratum protocol server for ASIC miners
3. **Simulation Worker** - Simulated mining with configurable hash rate
4. **Constant Delay Worker** - Emits blocks at fixed intervals
5. **On-Demand Worker** - HTTP-triggered block mining
6. **External Worker** - Integration with external mining programs

### Test Options

```bash
./run-e2e-tests.sh [options]
  --duration SECONDS      Test duration per worker (default: 60)
  --workers 'TYPES'       Space-separated worker types (default: all)
  --haskell-image IMAGE   Haskell implementation Docker image
  --rust-image IMAGE      Rust implementation Docker image
  --help                  Show help message
```

### Examples

Test only CPU and Stratum workers for 2 minutes each:
```bash
./run-e2e-tests.sh --duration 120 --workers "cpu stratum"
```

Use custom Docker images:
```bash
./run-e2e-tests.sh \
  --haskell-image myregistry/chainweb-mining-client:latest \
  --rust-image myregistry/chainweb-mining-client-rs:latest
```

## Performance Benchmarks

The benchmark suite measures and compares performance metrics between implementations.

### Benchmark Options

```bash
./run-benchmarks.sh [options]
  --duration SECONDS   Benchmark duration (default: 300)
  --threads 'COUNTS'   Thread counts to test (default: '1 2 4 8')
  --workers 'COUNTS'   Worker counts to test (default: '1 2 4 8')
  --help              Show help message
```

### Metrics Collected

**CPU Worker:**
- Solutions found per thread configuration
- Hash rate (MH/s)
- CPU and memory utilization
- Efficiency (solutions per thread)

**Stratum Server:**
- Concurrent connections handled
- Shares submitted
- Connection throughput
- Shares per client ratio

### Example Output

```
CPU Worker Performance
| Threads | Haskell Solutions | Rust Solutions | Improvement % |
|---------|-------------------|----------------|---------------|
| 1       | 45                | 52             | 15.5%         |
| 2       | 89                | 104            | 16.8%         |
| 4       | 178               | 208            | 16.8%         |
| 8       | 352               | 415            | 17.9%         |
```

## Docker Compose Structure

The tests use a modular Docker Compose setup:

- `docker-compose.base.yml` - Base Chainweb nodes (with and without PoW)
- `docker-compose.cpu.yml` - CPU worker configuration
- `docker-compose.stratum.yml` - Stratum server configuration
- `docker-compose.simulation.yml` - Simulation worker configuration
- `docker-compose.constant-delay.yml` - Constant delay worker configuration
- `docker-compose.on-demand.yml` - On-demand worker configuration
- `docker-compose.external.yml` - External worker configuration

## Test Results

Results are saved in timestamped files:

- `results/e2e_test_results_TIMESTAMP.json` - Raw E2E test data
- `results/e2e_test_summary_TIMESTAMP.txt` - Human-readable E2E summary
- `benchmarks/benchmark_results_TIMESTAMP.json` - Raw benchmark data
- `benchmarks/benchmark_report_TIMESTAMP.md` - Detailed benchmark report

## Running Individual Tests

You can run specific worker tests using Docker Compose directly:

```bash
cd docker

# Start base services
docker-compose -f docker-compose.base.yml up -d

# Run CPU worker test
docker-compose -f docker-compose.base.yml -f docker-compose.cpu.yml up

# Clean up
docker-compose -f docker-compose.base.yml down -v
```

## Integration Tests (Rust)

The Rust implementation includes integration tests that can be run with:

```bash
cd ../..  # Go to rust project root
cargo test --test integration_tests -- --ignored --test-threads=1
```

These require running Chainweb nodes on the expected ports.

## Troubleshooting

### Common Issues

1. **Port conflicts**: Ensure ports 1848, 1849, 1917, 1918 are available
2. **Docker permissions**: Run with appropriate permissions or use sudo
3. **Resource limits**: Ensure Docker has sufficient CPU and memory allocated
4. **Image pull failures**: Check network connectivity and Docker Hub access

### Debug Mode

Enable debug logging by setting environment variables:
```bash
export RUST_LOG=debug
export LOG_LEVEL=debug
./run-e2e-tests.sh
```

### Manual Cleanup

If tests fail to clean up properly:
```bash
docker-compose -f docker/docker-compose.base.yml down -v
docker rm -f $(docker ps -aq --filter "label=benchmark=true")
docker network rm chainweb-test
```

## Contributing

When adding new tests:

1. Update the appropriate docker-compose file
2. Add test logic to the runner scripts
3. Update this README with new test documentation
4. Ensure proper cleanup in error scenarios