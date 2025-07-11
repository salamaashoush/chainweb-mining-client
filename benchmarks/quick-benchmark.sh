#!/bin/bash
# Quick benchmark script - runs after setup-and-build.sh

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}==================================="
echo "Quick Mining Performance Benchmark"
echo "===================================${NC}"

# Load configuration
if [ -f "benchmark-config.env" ]; then
    source benchmark-config.env
else
    echo "Please run setup-and-build.sh first!"
    exit 1
fi

# Check if executables exist
if [ ! -f "$HASKELL_MINING_CLIENT" ]; then
    echo "Haskell executable not found. Please run setup-and-build.sh first!"
    exit 1
fi

if [ ! -f "$RUST_MINING_CLIENT" ]; then
    echo "Rust executable not found. Please run setup-and-build.sh first!"
    exit 1
fi

# Create results directory
RESULTS_DIR="results/quick_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULTS_DIR"

echo -e "\n${YELLOW}Testing connection to node at $NODE_HOST...${NC}"
if curl -s "http://$NODE_HOST/chainweb/0.0/development/cut" > /dev/null; then
    echo -e "${GREEN}✓ Node is accessible${NC}"
else
    echo -e "${YELLOW}Warning: Cannot connect to node at $NODE_HOST${NC}"
    echo "Please ensure a Chainweb node is running:"
    echo "  docker run -p 1848:1848 kadena/chainweb-node:latest"
fi

# Function to run a quick test
run_quick_test() {
    local impl=$1
    local exe=$2
    local threads=$3
    local duration=30
    
    echo -e "\n${YELLOW}Testing $impl with $threads threads for $duration seconds...${NC}"
    
    # Prepare command based on implementation
    if [ "$impl" = "Haskell" ]; then
        CMD="$exe --node-api-host=$NODE_HOST \
                  --miner-account=$MINER_ACCOUNT \
                  --miner-public-key=$MINER_PUBLIC_KEY \
                  --worker=cpu \
                  --thread-count=$threads \
                  --log-level=info"
    else
        CMD="$exe --node $NODE_HOST \
                  --account $MINER_ACCOUNT \
                  --public-key $MINER_PUBLIC_KEY \
                  --worker cpu \
                  --threads $threads \
                  --log-level info"
    fi
    
    # Run the test
    echo "Command: $CMD" > "$RESULTS_DIR/${impl}_${threads}t_command.txt"
    
    timeout $duration $CMD 2>&1 | tee "$RESULTS_DIR/${impl}_${threads}t_output.log" &
    local PID=$!
    
    # Simple monitoring
    sleep 5  # Let it warm up
    
    # Take a few CPU/memory samples
    echo "Time,CPU%,Memory(MB)" > "$RESULTS_DIR/${impl}_${threads}t_stats.csv"
    for i in {1..5}; do
        if ps -p $PID > /dev/null 2>&1; then
            ps -o %cpu,rss -p $PID | tail -1 | awk -v t=$((i*5)) '{print t "," $1 "," $2/1024}' >> "$RESULTS_DIR/${impl}_${threads}t_stats.csv"
            sleep 5
        fi
    done
    
    wait $PID || true
    
    # Extract hash rate
    local hash_rate=$(grep -i "hash.*rate\|hashes.*per.*sec\|h/s" "$RESULTS_DIR/${impl}_${threads}t_output.log" | tail -1 | grep -oE '[0-9]+(\.[0-9]+)?' | tail -1)
    
    if [ -n "$hash_rate" ]; then
        echo -e "${GREEN}$impl Hash Rate: $hash_rate H/s${NC}"
        echo "$impl,$threads,$hash_rate" >> "$RESULTS_DIR/summary.csv"
    else
        echo -e "${YELLOW}Could not extract hash rate for $impl${NC}"
        echo "$impl,$threads,0" >> "$RESULTS_DIR/summary.csv"
    fi
}

# Write CSV header
echo "Implementation,Threads,HashRate" > "$RESULTS_DIR/summary.csv"

# Run tests with 4 threads (typical setup)
run_quick_test "Haskell" "$HASKELL_MINING_CLIENT" 4
echo -e "\n${YELLOW}Cooling down...${NC}"
sleep 5
run_quick_test "Rust" "$RUST_MINING_CLIENT" 4

# Show results
echo -e "\n${GREEN}==================================="
echo "Quick Benchmark Results"
echo "===================================${NC}"
echo
cat "$RESULTS_DIR/summary.csv" | column -t -s,
echo
echo "Detailed results saved to: $RESULTS_DIR"

# Calculate speedup if both have results
HASKELL_RATE=$(grep "Haskell" "$RESULTS_DIR/summary.csv" | cut -d, -f3)
RUST_RATE=$(grep "Rust" "$RESULTS_DIR/summary.csv" | cut -d, -f3)

if [ -n "$HASKELL_RATE" ] && [ -n "$RUST_RATE" ] && [ "$HASKELL_RATE" != "0" ]; then
    SPEEDUP=$(echo "scale=2; $RUST_RATE / $HASKELL_RATE" | bc)
    echo -e "\n${GREEN}Rust is ${SPEEDUP}x faster than Haskell${NC}"
fi