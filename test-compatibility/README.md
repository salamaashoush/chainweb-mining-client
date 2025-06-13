# Chainweb Mining Client Compatibility Tests

This directory contains comprehensive compatibility tests between the Haskell and Rust implementations of the chainweb-mining-client.

## Prerequisites

- Docker and Docker Compose
- Haskell chainweb-mining-client binary (built via cabal)
- Rust chainweb-mining-client binary (will be built automatically if missing)
- `expect` command for Stratum tests
- `nc` (netcat) for network tests
- `curl` for HTTP tests

## Test Scripts

### 1. `test-all-workers.sh`
Tests all worker types with both implementations:
- CPU worker
- Simulation worker  
- Constant-delay worker

Usage:
```bash
./test-all-workers.sh
./test-all-workers.sh --keep-nodes  # Don't stop Docker containers after tests
```

### 2. `test-stratum-worker.sh`
Tests Stratum protocol compatibility:
- Runs both Haskell and Rust Stratum servers
- Tests basic connectivity
- Runs the original expect script tests
- Compares protocol responses

Usage:
```bash
./test-stratum-worker.sh
```

### 3. `test-external-worker.sh`
Tests external worker functionality:
- Creates a mock external miner script
- Tests that both implementations can spawn and communicate with external processes
- Verifies work is passed correctly to external miners

Usage:
```bash
./test-external-worker.sh
```

### 4. `test-on-demand-worker.sh`
Tests the on-demand worker (Rust only):
- Starts an HTTP server for on-demand mining
- Tests the mining trigger endpoint
- Verifies mining can be triggered via HTTP

Usage:
```bash
./test-on-demand-worker.sh
```

## Docker Setup

Multiple docker-compose configurations are provided:

1. **docker-compose.yml**: Full configuration with two nodes
   - `chainweb-dev`: Development node with `DISABLE_POW_VALIDATION=1` (ports 1848/1789)
   - `chainweb-prod`: Production-like node (ports 1849/1790)

2. **docker-compose.simple.yml**: Simplified single-node setup for quick testing
   - Single node with mining enabled
   - Minimal configuration that's known to work
   - Use: `docker compose -f docker-compose.simple.yml up -d`

3. **docker-compose.minimal.yml**: Based on Kadena devnet minimal configuration
   - Requires minimal-config.yaml
   - Most similar to official devnet setup

All configurations:
- Use `salamaashoush/chainweb-node:latest` image
- Enable mining coordination
- Configure the test public key for mining rewards
- Include health checks and proper shutdown handling

## Running All Tests

To run the complete test suite:

```bash
# Make scripts executable
chmod +x *.sh

# Run all tests
./test-all-workers.sh
./test-stratum-worker.sh
./test-external-worker.sh
./test-on-demand-worker.sh
```

## Test Results

Each test script will output:
- ✅ Green checkmarks for passing tests
- ❌ Red X marks for failing tests
- 🔍 Blue magnifying glass for test steps
- ⚠️  Yellow warnings for non-critical issues

The scripts exit with code 0 on success and 1 on failure, making them suitable for CI/CD pipelines.

## Troubleshooting

If tests fail:

1. Check Docker logs:
   ```bash
   docker-compose logs chainweb-dev
   ```

2. Check test logs in `/tmp/`:
   - `/tmp/haskell_*.log`
   - `/tmp/rust_*.log`

3. Ensure the chainweb node has mining endpoints enabled:
   - For dev nodes: `DISABLE_POW_VALIDATION=1` must be set
   - Check `http://localhost:1848/info` for node status

4. For Stratum tests, ensure ports 1917-1918 are available

5. For on-demand tests, ensure port 9090 is available

## Notes

- The Haskell implementation uses `--public-keys` (plural) while Rust uses `--public-key` (singular)
- Some worker types are Rust-only (on-demand) or have different implementations
- The test scripts handle these differences automatically