#!/bin/bash
# One-command benchmark runner

echo "======================================"
echo "Chainweb Mining Client Benchmark Suite"
echo "======================================"
echo
echo "This script will:"
echo "1. Build both Haskell and Rust implementations"
echo "2. Run performance benchmarks"
echo "3. Generate comparison report"
echo
read -p "Continue? (y/n) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    exit 0
fi

# Make scripts executable
chmod +x benchmarks/*.sh

# Run setup and build
echo -e "\n>>> Step 1: Building implementations..."
cd benchmarks && ./setup-and-build.sh
if [ $? -ne 0 ]; then
    echo "Build failed! Please check the error messages above."
    exit 1
fi

# Check if node is running
echo -e "\n>>> Step 2: Checking for Chainweb node..."
if ! curl -s http://localhost:1848/chainweb/0.0/development/cut > /dev/null 2>&1; then
    echo "No Chainweb node found. Starting one with Docker..."
    docker run -d --name chainweb-benchmark -p 1848:1848 kadena/chainweb-node:latest
    echo "Waiting for node to start..."
    sleep 10
fi

# Run quick benchmark
echo -e "\n>>> Step 3: Running benchmarks..."
./quick-benchmark.sh

echo -e "\n======================================"
echo "Benchmark complete!"
echo "======================================"
echo
echo "To run more comprehensive benchmarks:"
echo "  cd benchmarks"
echo "  ./cpu-benchmark.sh    # Full CPU test (30 min)"
echo "  ./memory-benchmark.sh # Memory usage test (10 min)"
echo
echo "To stop the Chainweb node:"
echo "  docker stop chainweb-benchmark && docker rm chainweb-benchmark"
