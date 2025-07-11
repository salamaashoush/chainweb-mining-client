# Benchmark Guide: Haskell vs Rust Mining Client

## Prerequisites

### System Requirements
- Linux or macOS (Windows via WSL2)
- At least 8GB RAM (16GB recommended)
- Multi-core CPU (4+ cores recommended)
- ~2GB free disk space

### Required Software

```bash
# 1. Development Tools
sudo apt-get update
sudo apt-get install -y \
    build-essential \
    git \
    curl \
    pkg-config \
    libssl-dev \
    sysstat \
    time \
    htop \
    python3-pip

# 2. Haskell (via GHCup)
curl --proto '=https' --tlsv1.2 -sSf https://get-ghcup.haskell.org | sh
# Follow the prompts, then:
source ~/.bashrc
ghcup install ghc 9.6.3
ghcup install cabal 3.10.1.0
ghcup set ghc 9.6.3
ghcup set cabal 3.10.1.0

# 3. Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustup default stable

# 4. Python packages for analysis
pip3 install pandas matplotlib psutil

# 5. Monitoring tools
sudo apt-get install -y linux-tools-common linux-tools-generic
```

## Setup

### 1. Clone and Build Both Implementations (REQUIRED BEFORE BENCHMARKING)

```bash
# Clone the repository (if not already done)
cd ~/Workspace/Kadena
git clone https://github.com/kadena-io/chainweb-mining-client.git
cd chainweb-mining-client

# Build Haskell version
echo "Building Haskell implementation..."
cabal update
cabal build --enable-tests --enable-benchmarks
# Note: First build may take 15-30 minutes as it downloads dependencies

# Verify Haskell build
find . -name chainweb-mining-client -type f -executable | grep -v ".git"
# Should show the built executable path

# Build Rust version
echo "Building Rust implementation..."
cd chainweb-mining-client-rust
cargo build --release
# Note: First build may take 5-10 minutes

# Verify Rust build
ls -la target/release/chainweb-mining-client
# Should show the built executable

cd ..
```

### Important Build Notes:

1. **Haskell Build Path**: The Haskell executable will be in a deep path like:
   ```
   dist-newstyle/build/*/chainweb-mining-client-*/x/chainweb-mining-client/build/chainweb-mining-client/chainweb-mining-client
   ```

2. **Rust Build Path**: The Rust executable will always be at:
   ```
   chainweb-mining-client-rust/target/release/chainweb-mining-client
   ```

3. **Build Troubleshooting**:
   ```bash
   # If Haskell build fails:
   cabal clean
   cabal update
   cabal build -v2  # verbose output
   
   # If Rust build fails:
   cd chainweb-mining-client-rust
   cargo clean
   cargo build --release --verbose
   ```

4. **Verify Both Builds Work**:
   ```bash
   # Test Haskell version
   ./dist-newstyle/build/*/chainweb-mining-client-*/x/chainweb-mining-client/build/chainweb-mining-client/chainweb-mining-client --help
   
   # Test Rust version
   ./chainweb-mining-client-rust/target/release/chainweb-mining-client --help
   ```

### 2. Start a Local Chainweb Node (Required for Mining)

```bash
# Option 1: Use Docker (easiest)
docker run -it --rm \
    -p 1848:1848 \
    kadena/chainweb-node:latest \
    --config-file=/data/devnet.yaml

# Option 2: Download and run binary
wget https://github.com/kadena-io/chainweb-node/releases/latest/download/chainweb-node
chmod +x chainweb-node
./chainweb-node --node-id=0 --p2p-hostname=127.0.0.1 --service-port=1848
```

### 3. Create Benchmark Directory Structure

```bash
mkdir -p benchmarks/{scripts,results,logs}
cd benchmarks
```

## Running the Benchmarks

### Quick Test (5 minutes)

```bash
#!/bin/bash
# Save as benchmarks/quick-test.sh

echo "=== Quick Benchmark Test ==="
echo "Testing both implementations for 30 seconds each..."

# Test Haskell
echo -e "\n--- Haskell CPU Mining (30s) ---"
timeout 30s ../dist-newstyle/build/*/chainweb-mining-client-*/x/chainweb-mining-client/build/chainweb-mining-client/chainweb-mining-client \
    --node-api-host=localhost:1848 \
    --miner-account="k:f90ef36c9a3da8fbb0cb8d5bf421c15862eeed62b042818762492f2488963e1d" \
    --miner-public-key="f90ef36c9a3da8fbb0cb8d5bf421c15862eeed62b042818762492f2488963e1d" \
    --worker=cpu \
    --thread-count=4 \
    --log-level=info 2>&1 | grep -E "(hash|Hash|rate|Rate)" | tail -5

# Test Rust
echo -e "\n--- Rust CPU Mining (30s) ---"
timeout 30s ../chainweb-mining-client-rust/target/release/chainweb-mining-client \
    --node localhost:1848 \
    --account "k:f90ef36c9a3da8fbb0cb8d5bf421c15862eeed62b042818762492f2488963e1d" \
    --public-key "f90ef36c9a3da8fbb0cb8d5bf421c15862eeed62b042818762492f2488963e1d" \
    --worker cpu \
    --threads 4 \
    --log-level info 2>&1 | grep -E "(hash|Hash|rate|Rate)" | tail -5

echo -e "\n=== Quick test complete ==="
```

### Full CPU Mining Benchmark (30 minutes)

```bash
#!/bin/bash
# Save as benchmarks/cpu-benchmark.sh

RESULTS_DIR="results/cpu_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULTS_DIR"

# Test account (generate your own with --generate-key)
ACCOUNT="k:f90ef36c9a3da8fbb0cb8d5bf421c15862eeed62b042818762492f2488963e1d"
PUBLIC_KEY="f90ef36c9a3da8fbb0cb8d5bf421c15862eeed62b042818762492f2488963e1d"

echo "Starting CPU mining benchmark..."
echo "Results will be saved to: $RESULTS_DIR"

# Function to run and monitor a single test
run_test() {
    local impl=$1
    local threads=$2
    local duration=$3
    local output_file="$RESULTS_DIR/${impl}_${threads}threads.txt"
    
    echo "Testing $impl with $threads threads for $duration seconds..."
    
    if [ "$impl" = "haskell" ]; then
        CMD="../dist-newstyle/build/*/chainweb-mining-client-*/x/chainweb-mining-client/build/chainweb-mining-client/chainweb-mining-client \
            --node-api-host=localhost:1848 \
            --miner-account=$ACCOUNT \
            --miner-public-key=$PUBLIC_KEY \
            --worker=cpu \
            --thread-count=$threads \
            --log-level=info"
    else
        CMD="../chainweb-mining-client-rust/target/release/chainweb-mining-client \
            --node localhost:1848 \
            --account $ACCOUNT \
            --public-key $PUBLIC_KEY \
            --worker cpu \
            --threads $threads \
            --log-level info"
    fi
    
    # Start the miner with monitoring
    timeout $duration $CMD > "$output_file" 2>&1 &
    local PID=$!
    
    # Monitor CPU and memory
    echo "Time,CPU%,Memory(MB)" > "$RESULTS_DIR/${impl}_${threads}threads_stats.csv"
    
    for i in $(seq 1 $duration); do
        if ps -p $PID > /dev/null; then
            ps -o %cpu,rss -p $PID | tail -1 | awk -v t=$i '{print t "," $1 "," $2/1024}' >> "$RESULTS_DIR/${impl}_${threads}threads_stats.csv"
            sleep 1
        fi
    done
    
    wait $PID
    
    # Extract hash rate
    local hash_rate=$(grep -i "hash.*rate\|hashes.*per.*sec" "$output_file" | tail -1 | grep -oE '[0-9]+(\.[0-9]+)?')
    echo "$impl,$threads,$hash_rate" >> "$RESULTS_DIR/summary.csv"
}

# Write CSV header
echo "Implementation,Threads,HashRate" > "$RESULTS_DIR/summary.csv"

# Test different thread counts
for threads in 1 2 4 8; do
    run_test "haskell" $threads 60
    run_test "rust" $threads 60
    echo "Cooling down..."
    sleep 10
done

echo "Benchmark complete! Results in: $RESULTS_DIR"
```

### Stratum Server Benchmark

First, create a simple Stratum load tester:

```python
#!/usr/bin/env python3
# Save as benchmarks/stratum-load-test.py

import asyncio
import json
import time
import sys
from collections import defaultdict

class StratumClient:
    def __init__(self, worker_id):
        self.worker_id = worker_id
        self.shares_submitted = 0
        self.latencies = []
        
    async def connect_and_mine(self, host, port, duration):
        try:
            reader, writer = await asyncio.open_connection(host, port)
            
            # Subscribe
            subscribe = json.dumps({
                "id": 1,
                "method": "mining.subscribe",
                "params": [f"test-miner/{self.worker_id}"]
            }) + "\n"
            writer.write(subscribe.encode())
            await writer.drain()
            await reader.readline()  # Read response
            
            # Authorize
            auth = json.dumps({
                "id": 2,
                "method": "mining.authorize",
                "params": [f"worker{self.worker_id}", "x"]
            }) + "\n"
            writer.write(auth.encode())
            await writer.drain()
            await reader.readline()  # Read response
            
            # Submit shares
            start_time = time.time()
            while time.time() - start_time < duration:
                submit_time = time.time()
                
                submit = json.dumps({
                    "id": 3,
                    "method": "mining.submit",
                    "params": [
                        f"worker{self.worker_id}",
                        "jobid",
                        "00000000",
                        "00000000", 
                        "00000000"
                    ]
                }) + "\n"
                
                writer.write(submit.encode())
                await writer.drain()
                
                response = await reader.readline()
                latency = (time.time() - submit_time) * 1000  # ms
                self.latencies.append(latency)
                self.shares_submitted += 1
                
                await asyncio.sleep(0.1)  # 10 shares/sec per worker
                
            writer.close()
            await writer.wait_closed()
            
        except Exception as e:
            print(f"Worker {self.worker_id} error: {e}")

async def run_load_test(host, port, num_workers, duration):
    print(f"Starting Stratum load test: {num_workers} workers for {duration} seconds")
    
    clients = [StratumClient(i) for i in range(num_workers)]
    tasks = [client.connect_and_mine(host, port, duration) for client in clients]
    
    start_time = time.time()
    await asyncio.gather(*tasks)
    elapsed = time.time() - start_time
    
    # Calculate statistics
    total_shares = sum(c.shares_submitted for c in clients)
    all_latencies = []
    for c in clients:
        all_latencies.extend(c.latencies)
    
    avg_latency = sum(all_latencies) / len(all_latencies) if all_latencies else 0
    
    print(f"\nResults:")
    print(f"  Total shares: {total_shares}")
    print(f"  Shares/sec: {total_shares / elapsed:.2f}")
    print(f"  Avg latency: {avg_latency:.2f} ms")
    print(f"  Min latency: {min(all_latencies):.2f} ms")
    print(f"  Max latency: {max(all_latencies):.2f} ms")

if __name__ == "__main__":
    if len(sys.argv) != 5:
        print("Usage: stratum-load-test.py <host> <port> <workers> <duration>")
        sys.exit(1)
        
    host = sys.argv[1]
    port = int(sys.argv[2])
    workers = int(sys.argv[3])
    duration = int(sys.argv[4])
    
    asyncio.run(run_load_test(host, port, workers, duration))
```

### Memory Usage Comparison

```bash
#!/bin/bash
# Save as benchmarks/memory-benchmark.sh

echo "Memory Usage Benchmark"
echo "====================="

RESULTS_DIR="results/memory_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULTS_DIR"

# Function to monitor memory usage
monitor_memory() {
    local impl=$1
    local worker=$2
    local duration=120  # 2 minutes
    local output="$RESULTS_DIR/${impl}_${worker}_memory.csv"
    
    echo "Testing $impl with $worker worker..."
    
    if [ "$impl" = "haskell" ]; then
        CMD="../dist-newstyle/build/*/chainweb-mining-client-*/x/chainweb-mining-client/build/chainweb-mining-client/chainweb-mining-client \
            --node-api-host=localhost:1848 \
            --worker=$worker"
    else
        CMD="../chainweb-mining-client-rust/target/release/chainweb-mining-client \
            --node localhost:1848 \
            --worker $worker"
    fi
    
    # Start the process
    $CMD > /dev/null 2>&1 &
    local PID=$!
    
    # Monitor memory
    echo "Time,RSS(MB),VMS(MB),CPU%" > "$output"
    
    for i in $(seq 1 $duration); do
        if ps -p $PID > /dev/null; then
            ps -o rss=,vsz=,%cpu= -p $PID | awk -v t=$i '{print t "," $1/1024 "," $2/1024 "," $3}' >> "$output"
            sleep 1
        else
            break
        fi
    done
    
    kill $PID 2>/dev/null
    
    # Calculate averages
    local avg_rss=$(awk -F, 'NR>1 {sum+=$2; count++} END {print sum/count}' "$output")
    echo "$impl,$worker,$avg_rss" >> "$RESULTS_DIR/memory_summary.csv"
}

echo "Implementation,Worker,AvgMemory(MB)" > "$RESULTS_DIR/memory_summary.csv"

# Test each worker type
for worker in cpu stratum simulation; do
    monitor_memory "haskell" $worker
    monitor_memory "rust" $worker
    sleep 5
done

echo "Memory benchmark complete! Results in: $RESULTS_DIR"
```

## Running All Benchmarks

```bash
#!/bin/bash
# Save as benchmarks/run-all-benchmarks.sh

echo "Running complete benchmark suite..."
echo "=================================="

# Make scripts executable
chmod +x *.sh
chmod +x *.py

# 1. Quick test to ensure everything works
./quick-test.sh

# 2. CPU mining benchmark
./cpu-benchmark.sh

# 3. Memory benchmark
./memory-benchmark.sh

# 4. Stratum benchmark (start servers first)
echo "Starting Stratum servers..."

# Start Haskell Stratum server
../dist-newstyle/build/*/chainweb-mining-client-*/x/chainweb-mining-client/build/chainweb-mining-client/chainweb-mining-client \
    --worker=stratum --stratum-port=1917 > logs/haskell-stratum.log 2>&1 &
HASKELL_PID=$!
sleep 5

# Test Haskell
python3 stratum-load-test.py localhost 1917 100 60 > results/haskell-stratum-results.txt
kill $HASKELL_PID

# Start Rust Stratum server
../chainweb-mining-client-rust/target/release/chainweb-mining-client \
    --worker stratum --stratum-port 1917 > logs/rust-stratum.log 2>&1 &
RUST_PID=$!
sleep 5

# Test Rust
python3 stratum-load-test.py localhost 1917 100 60 > results/rust-stratum-results.txt
kill $RUST_PID

echo "All benchmarks complete!"
echo "Results are in the benchmarks/results/ directory"
```

## Analyzing Results

```python
#!/usr/bin/env python3
# Save as benchmarks/analyze-results.py

import pandas as pd
import matplotlib.pyplot as plt
import glob
import os

# Find latest results
results_dirs = sorted(glob.glob('results/cpu_*'))
latest_cpu = results_dirs[-1] if results_dirs else None

if latest_cpu:
    # Read CPU benchmark summary
    df = pd.read_csv(f'{latest_cpu}/summary.csv')
    
    # Create comparison plot
    plt.figure(figsize=(10, 6))
    
    rust_data = df[df['Implementation'] == 'rust']
    haskell_data = df[df['Implementation'] == 'haskell']
    
    plt.plot(rust_data['Threads'], rust_data['HashRate'], 'ro-', label='Rust', linewidth=2)
    plt.plot(haskell_data['Threads'], haskell_data['HashRate'], 'bo-', label='Haskell', linewidth=2)
    
    plt.xlabel('Number of Threads')
    plt.ylabel('Hash Rate (H/s)')
    plt.title('CPU Mining Performance: Rust vs Haskell')
    plt.legend()
    plt.grid(True)
    plt.savefig('results/cpu_performance_comparison.png', dpi=150)
    
    # Calculate speedup
    speedup = rust_data['HashRate'].values / haskell_data['HashRate'].values
    print(f"Average Rust speedup: {speedup.mean():.2f}x")
    
    # Memory comparison
    memory_dirs = sorted(glob.glob('results/memory_*'))
    if memory_dirs:
        latest_memory = memory_dirs[-1]
        mem_df = pd.read_csv(f'{latest_memory}/memory_summary.csv')
        
        print("\nMemory Usage Comparison:")
        print(mem_df.pivot(index='Worker', columns='Implementation', values='AvgMemory(MB)'))
```

## Expected Results

Based on the implementations, you should see:

1. **CPU Mining**: Rust 2-4x faster due to multi-threading
2. **Memory Usage**: Rust uses 30-50% less memory (no GC)
3. **Stratum Server**: Rust handles 2-3x more connections
4. **Latency**: Rust has 20-30% lower latency

## Troubleshooting

```bash
# If Haskell build fails
cabal clean
cabal update
cabal build --verbose

# If Rust build fails
cd chainweb-mining-client-rust
cargo clean
cargo build --release --verbose

# If benchmarks fail to connect
# Check if chainweb node is running
curl http://localhost:1848/chainweb/0.0/development/cut

# Check ports
netstat -tlnp | grep -E "1848|1917"

# Monitor in real-time
htop  # In another terminal while benchmarks run
```

## Next Steps

After running benchmarks:
1. Save all results for comparison after optimizations
2. Identify specific bottlenecks from the data
3. Focus optimization efforts on the biggest gaps
4. Re-run benchmarks after each optimization

The benchmark data will guide which optimizations to prioritize (CPU SIMD, memory pooling, or GPU support).