#!/bin/bash

# Performance test script for Chainweb Mining Client
# Compares hash rates between different configurations

echo "Chainweb Mining Client - Performance Test"
echo "========================================="
echo ""

# Build in release mode
echo "Building release version..."
cargo build --release --quiet

# Function to test CPU mining with different thread counts
test_cpu_threads() {
    local threads=$1
    echo "Testing CPU mining with $threads threads..."
    
    # Create test config
    cat > test-config-$threads.toml <<EOF
[node]
url = "api.chainweb.com"
use_tls = true
timeout_secs = 30
chain_id = 0

[mining]
account = "test-account"
public_key = "0000000000000000000000000000000000000000000000000000000000000000"
update_interval_secs = 5

[worker]
type = "cpu"
threads = $threads
batch_size = 100000

[logging]
level = "info"
format = "plain"
EOF

    # Run for 30 seconds and capture output
    timeout 30s ./target/release/chainweb-mining-client --config test-config-$threads.toml 2>&1 | grep -i "hashrate" | tail -5
    
    # Clean up
    rm -f test-config-$threads.toml
    echo ""
}

# Test with different thread counts
echo "Running performance tests..."
echo ""

# Single thread
test_cpu_threads 1

# Half of available cores
HALF_CORES=$(($(nproc) / 2))
test_cpu_threads $HALF_CORES

# All cores
test_cpu_threads 0

echo "Performance test complete!"