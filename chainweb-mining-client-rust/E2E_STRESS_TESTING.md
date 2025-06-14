# End-to-End Stress Testing Guide

This document provides comprehensive instructions for performing end-to-end stress testing of the Chainweb Mining Client with a real chainweb node.

## Overview

The end-to-end stress testing framework tests the complete mining client stack under realistic conditions:

- **Real Chainweb Node**: Uses Docker to run an actual chainweb node
- **Compiled CLI**: Tests the release build of the mining client
- **Production Monitoring**: Validates the integrated monitoring system
- **Multiple Worker Types**: Tests CPU, Stratum, and External workers
- **High Load Scenarios**: Stress tests with multiple concurrent workers
- **Health Monitoring**: Continuous node health checks during testing

## Prerequisites

### System Requirements
- **Docker**: For running chainweb node
- **Rust**: For building the mining client
- **curl**: For API calls and health checks
- **jq**: For JSON processing
- **bash**: For running test scripts

### Hardware Recommendations
- **CPU**: 4+ cores (for concurrent worker testing)
- **RAM**: 8GB+ (chainweb node requires ~2GB)
- **Storage**: 10GB+ free space
- **Network**: Stable internet connection

## Quick Start

### 1. Automated Stress Testing

Run the comprehensive stress test script:

```bash
cd chainweb-mining-client-rust

# Run with default settings (60s per test, 4 workers)
./scripts/e2e-stress-test.sh

# Run with custom settings
TEST_DURATION=120 WORKER_COUNT=8 ./scripts/e2e-stress-test.sh
```

### 2. Manual Testing

#### Start Chainweb Node
```bash
# Navigate to test directory
cd ../test-compatibility

# Start development node (PoW validation disabled)
./start-chainweb-node.sh dev

# Wait for node to be ready
curl http://localhost:1848/info
```

#### Build Mining Client
```bash
cd ../chainweb-mining-client-rust
cargo build --release
```

#### Run Individual Tests
```bash
# CPU mining test
./target/release/chainweb-mining-client cpu \
    --chainweb-url http://localhost:1848 \
    --account test-miner \
    --threads 4 \
    --batch-size 100000

# Stratum server test
./target/release/chainweb-mining-client stratum \
    --chainweb-url http://localhost:1848 \
    --account test-miner \
    --port 1917

# External worker test
./target/release/chainweb-mining-client external \
    --chainweb-url http://localhost:1848 \
    --account test-miner \
    --command "echo 'mock-external-worker'"
```

## Advanced Testing

### 3. Programmatic Testing

Use the Rust test framework for detailed testing:

```bash
# Run basic end-to-end tests (requires Docker)
cargo test test_e2e_cpu_mining_stress --ignored

# Run comprehensive stress test (long-running)
cargo test test_e2e_comprehensive_stress --ignored

# Run all stress tests
cargo test e2e_ --ignored
```

### 4. Monitoring Integration

The monitoring system is automatically integrated and provides:

#### Real-time Metrics
- Hash rate tracking
- Share acceptance rates
- Response time monitoring
- Memory and CPU usage
- Solution detection

#### Monitoring Commands
```bash
# View current monitoring status
./target/release/chainweb-mining-client --monitoring-status

# View detailed metrics during mining
./target/release/chainweb-mining-client cpu \
    --chainweb-url http://localhost:1848 \
    --account test-miner \
    --log-level debug
```

#### Health Checks
The monitoring system automatically:
- Tracks performance metrics
- Alerts on threshold violations
- Records mining statistics
- Monitors node connectivity

## Test Scenarios

### 5. CPU Mining Stress Test

**Objective**: Validate CPU mining performance under load

**Configuration**:
- Multiple concurrent CPU workers
- Large batch sizes (50K-500K)
- Extended runtime (60+ seconds)
- Resource monitoring

**Expected Results**:
- Consistent hash rates
- No memory leaks
- Stable node connection
- Accurate solution detection

### 6. Stratum Server Stress Test

**Objective**: Test Stratum protocol under high connection load

**Configuration**:
- Single Stratum server
- Multiple simulated ASIC connections
- Concurrent share submissions
- Protocol compliance testing

**Expected Results**:
- Successful miner connections
- Proper job distribution
- Share acceptance/rejection
- Protocol message handling

### 7. External Worker Stress Test

**Objective**: Validate external worker integration

**Configuration**:
- Multiple external worker processes
- Mock external mining commands
- Timeout handling
- Error recovery

**Expected Results**:
- Successful process spawning
- Proper work distribution
- Timeout compliance
- Error handling

### 8. Comprehensive Stress Test

**Objective**: End-to-end system validation

**Configuration**:
- All worker types sequentially
- Extended runtime (5+ minutes)
- Continuous health monitoring
- Performance benchmarking

**Expected Results**:
- All workers function correctly
- Node remains stable
- No resource exhaustion
- Monitoring data collection

## Performance Benchmarks

### Expected Performance Metrics

| Metric | Target | Measured |
|--------|--------|----------|
| CPU Hash Rate | >100K H/s per core | TBD |
| Stratum Connections | >100 concurrent | TBD |
| Memory Usage | <500MB total | TBD |
| Response Time | <100ms average | TBD |
| Success Rate | >99% | TBD |

### Monitoring Thresholds

The monitoring system uses these default thresholds:
- **Min Hash Rate**: 1,000 H/s
- **Max Response Time**: 10,000ms
- **Min Acceptance Rate**: 90%
- **Max Memory Usage**: 1GB
- **Max CPU Usage**: 95%

## Troubleshooting

### Common Issues

#### Docker Node Startup
```bash
# Check if Docker is running
docker info

# Check container status
docker ps -a

# View node logs
docker logs chainweb-mining-test

# Restart node
docker stop chainweb-mining-test
docker rm chainweb-mining-test
./start-chainweb-node.sh dev
```

#### Mining Client Issues
```bash
# Check build status
cargo check

# View detailed logs
RUST_LOG=debug ./target/release/chainweb-mining-client ...

# Check monitoring status
./target/release/chainweb-mining-client --monitoring-status
```

#### Network Issues
```bash
# Test node connectivity
curl http://localhost:1848/info

# Check port availability
netstat -tlnp | grep 1848

# Test API endpoints
curl -X POST http://localhost:1848/chainweb/0.0/development/mining/work \
  -H "Content-Type: application/json" \
  -d '{"account":"test","predicate":"keys-all","public-keys":[]}'
```

### Performance Issues

#### Low Hash Rates
- Increase batch size
- Adjust thread count
- Check CPU utilization
- Verify no resource contention

#### High Memory Usage
- Monitor buffer pools
- Check for memory leaks
- Adjust batch sizes
- Review allocation patterns

#### Connection Problems
- Verify node health
- Check network connectivity
- Review timeout settings
- Monitor connection pooling

## Test Results Analysis

### Log Analysis

Test logs are generated in the current directory:
- `stress_test_cpu_*.log`: CPU worker logs
- `stress_test_stratum_*.log`: Stratum server logs
- `stress_test_external_*.log`: External worker logs
- `stress_test_summary.txt`: Overall summary

### Monitoring Data

The monitoring system collects:
- Performance metrics over time
- Alert history
- Health check results
- Resource utilization

### Success Criteria

A successful stress test should demonstrate:
- ✅ All workers start and run without errors
- ✅ Node remains healthy throughout testing
- ✅ Performance metrics meet targets
- ✅ No memory leaks or resource exhaustion
- ✅ Monitoring system functions correctly
- ✅ Error handling works as expected

## Production Readiness

The stress testing validates:

### Reliability
- Extended operation (hours/days)
- Error recovery
- Connection resilience
- Resource management

### Performance
- Hash rate optimization
- Memory efficiency
- Network utilization
- Concurrent operation

### Monitoring
- Real-time metrics
- Alert generation
- Health tracking
- Performance analysis

### Scalability
- Multiple workers
- High connection counts
- Large batch processing
- Extended operation

## Continuous Integration

### Automated Testing

The stress tests can be integrated into CI/CD:

```yaml
# GitHub Actions example
- name: E2E Stress Testing
  run: |
    cd chainweb-mining-client-rust
    TEST_DURATION=30 WORKER_COUNT=2 ./scripts/e2e-stress-test.sh
```

### Performance Regression

Monitor for:
- Hash rate degradation
- Memory usage increases
- Response time growth
- Error rate increases

## Conclusion

The end-to-end stress testing framework provides comprehensive validation of the Chainweb Mining Client under realistic production conditions. It tests all major components, validates monitoring integration, and ensures reliable operation under load.

For additional support or questions, refer to the main project documentation or create an issue in the GitHub repository.