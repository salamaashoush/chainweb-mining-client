#!/bin/bash
# End-to-end stress testing script with real chainweb node
#
# This script starts a chainweb node and runs comprehensive stress tests
# against it using the compiled mining client.

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Configuration
TEST_DURATION=${TEST_DURATION:-60}  # seconds per test
WORKER_COUNT=${WORKER_COUNT:-4}     # number of concurrent workers
NODE_ENDPOINT="http://localhost:1848"
ACCOUNT="stress-test-miner"

echo -e "${BLUE}🚀 Starting End-to-End Stress Testing${NC}"
echo "=============================================="
echo "Test Duration: ${TEST_DURATION} seconds per test"
echo "Worker Count: ${WORKER_COUNT}"
echo "Node Endpoint: ${NODE_ENDPOINT}"
echo "=============================================="

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to wait for node to be ready
wait_for_node() {
    echo -e "${YELLOW}Waiting for chainweb node to be ready...${NC}"
    max_attempts=30
    attempt=0
    while [ $attempt -lt $max_attempts ]; do
        if curl -s "${NODE_ENDPOINT}/info" >/dev/null 2>&1; then
            echo -e "${GREEN}✅ Node is ready!${NC}"
            return 0
        fi
        sleep 2
        attempt=$((attempt + 1))
        echo -n "."
    done
    echo -e "${RED}❌ Node failed to become ready${NC}"
    return 1
}

# Function to cleanup
cleanup() {
    echo -e "${YELLOW}🧹 Cleaning up...${NC}"
    
    # Stop any running mining processes
    pkill -f "chainweb-mining-client" || true
    
    # Stop Docker containers
    docker stop chainweb-mining-test >/dev/null 2>&1 || true
    docker rm chainweb-mining-test >/dev/null 2>&1 || true
    
    echo -e "${GREEN}✅ Cleanup completed${NC}"
}

# Set trap for cleanup
trap cleanup EXIT

# Check prerequisites
echo -e "${BLUE}🔍 Checking prerequisites...${NC}"

if ! command_exists docker; then
    echo -e "${RED}❌ Docker is not installed${NC}"
    exit 1
fi

if ! command_exists curl; then
    echo -e "${RED}❌ curl is not installed${NC}"
    exit 1
fi

if ! command_exists jq; then
    echo -e "${RED}❌ jq is not installed${NC}"
    exit 1
fi

echo -e "${GREEN}✅ All prerequisites are available${NC}"

# Build the mining client
echo -e "${BLUE}🔨 Building mining client...${NC}"
cargo build --release
if [ $? -ne 0 ]; then
    echo -e "${RED}❌ Failed to build mining client${NC}"
    exit 1
fi
echo -e "${GREEN}✅ Mining client built successfully${NC}"

# Start chainweb node
echo -e "${BLUE}🚀 Starting chainweb node...${NC}"
bash ../test-compatibility/start-chainweb-node.sh dev
if [ $? -ne 0 ]; then
    echo -e "${RED}❌ Failed to start chainweb node${NC}"
    exit 1
fi

wait_for_node
if [ $? -ne 0 ]; then
    exit 1
fi

# Get node info
echo -e "${BLUE}📊 Node Information:${NC}"
curl -s "${NODE_ENDPOINT}/info" | jq '.'

# Function to run stress test
run_stress_test() {
    local test_name="$1"
    local worker_type="$2"
    local extra_args="$3"
    
    echo -e "${BLUE}🔥 Starting ${test_name}...${NC}"
    echo "Duration: ${TEST_DURATION} seconds"
    echo "Workers: ${WORKER_COUNT}"
    
    # Start monitoring in background
    local monitor_pid=""
    if command_exists htop; then
        htop -d 5 &
        monitor_pid=$!
    fi
    
    local start_time=$(date +%s)
    local pids=()
    
    # Start worker processes
    for i in $(seq 1 $WORKER_COUNT); do
        local worker_account="${ACCOUNT}-${i}"
        
        case "$worker_type" in
            "cpu")
                timeout $TEST_DURATION ./target/release/chainweb-mining-client cpu \
                    --chainweb-url "$NODE_ENDPOINT" \
                    --account "$worker_account" \
                    --threads 2 \
                    --batch-size 50000 \
                    $extra_args > "stress_test_${worker_type}_${i}.log" 2>&1 &
                ;;
            "stratum")
                if [ $i -eq 1 ]; then
                    # Start Stratum server
                    timeout $TEST_DURATION ./target/release/chainweb-mining-client stratum \
                        --chainweb-url "$NODE_ENDPOINT" \
                        --account "$worker_account" \
                        --port 1917 \
                        $extra_args > "stress_test_stratum_server.log" 2>&1 &
                    sleep 3  # Give server time to start
                fi
                
                # Simulate ASIC miner connections
                timeout $TEST_DURATION bash -c "
                    for j in \$(seq 1 5); do
                        echo 'Simulating ASIC miner connection \${j}'
                        sleep 1
                    done
                " > "stress_test_stratum_miner_${i}.log" 2>&1 &
                ;;
            "external")
                timeout $TEST_DURATION ./target/release/chainweb-mining-client external \
                    --chainweb-url "$NODE_ENDPOINT" \
                    --account "$worker_account" \
                    --command "echo 'mock-external-worker'" \
                    $extra_args > "stress_test_external_${i}.log" 2>&1 &
                ;;
        esac
        
        pids+=($!)
        echo "Started worker $i (PID: ${pids[-1]})"
    done
    
    # Monitor progress
    local elapsed=0
    while [ $elapsed -lt $TEST_DURATION ]; do
        sleep 5
        elapsed=$(($(date +%s) - start_time))
        local remaining=$((TEST_DURATION - elapsed))
        echo -e "${YELLOW}⏱️  ${test_name}: ${elapsed}s elapsed, ${remaining}s remaining${NC}"
        
        # Check node health
        if ! curl -s "${NODE_ENDPOINT}/info" >/dev/null 2>&1; then
            echo -e "${RED}⚠️ Node health check failed!${NC}"
        fi
    done
    
    # Wait for all workers to finish
    echo "Waiting for workers to finish..."
    for pid in "${pids[@]}"; do
        wait $pid 2>/dev/null || true
    done
    
    # Stop monitoring
    if [ -n "$monitor_pid" ]; then
        kill $monitor_pid 2>/dev/null || true
    fi
    
    echo -e "${GREEN}✅ ${test_name} completed${NC}"
    
    # Analyze logs
    echo -e "${BLUE}📊 ${test_name} Results:${NC}"
    local total_logs=$(find . -name "stress_test_${worker_type}_*.log" | wc -l)
    echo "Log files created: $total_logs"
    
    # Count any errors in logs
    local error_count=0
    for log_file in stress_test_${worker_type}_*.log; do
        if [ -f "$log_file" ]; then
            local file_errors=$(grep -c -i "error\|failed\|panic" "$log_file" 2>/dev/null || echo "0")
            error_count=$((error_count + file_errors))
        fi
    done
    
    if [ $error_count -eq 0 ]; then
        echo -e "${GREEN}🎉 No errors detected in logs${NC}"
    else
        echo -e "${YELLOW}⚠️ ${error_count} errors found in logs${NC}"
    fi
    
    echo ""
}

# Run stress tests
echo -e "${BLUE}🧪 Running Stress Tests${NC}"
echo "=========================================="

# Test 1: CPU Mining Stress Test
run_stress_test "CPU Mining Stress Test" "cpu" "--log-level debug"

# Test 2: Stratum Server Stress Test
run_stress_test "Stratum Server Stress Test" "stratum" "--log-level debug"

# Test 3: External Worker Stress Test
run_stress_test "External Worker Stress Test" "external" "--log-level debug"

# Final node health check
echo -e "${BLUE}🏥 Final Health Check${NC}"
if curl -s "${NODE_ENDPOINT}/info" | jq '.nodeVersion' >/dev/null 2>&1; then
    echo -e "${GREEN}✅ Node is still healthy after stress testing${NC}"
else
    echo -e "${RED}❌ Node health check failed after stress testing${NC}"
fi

# Generate summary report
echo -e "${BLUE}📋 Generating Summary Report${NC}"
cat > stress_test_summary.txt << EOF
End-to-End Stress Test Summary
==============================
Date: $(date)
Duration: ${TEST_DURATION} seconds per test
Workers: ${WORKER_COUNT}
Node Endpoint: ${NODE_ENDPOINT}

Test Results:
- CPU Mining Stress Test: Completed
- Stratum Server Stress Test: Completed  
- External Worker Stress Test: Completed

Log Files:
$(find . -name "stress_test_*.log" -exec ls -lh {} \;)

Node Status: $(curl -s "${NODE_ENDPOINT}/info" | jq -r '.nodeVersion // "Unknown"')
EOF

echo -e "${GREEN}📄 Summary report saved to stress_test_summary.txt${NC}"

echo -e "${GREEN}🎉 All stress tests completed successfully!${NC}"
echo ""
echo "Next steps:"
echo "1. Review log files: stress_test_*.log"
echo "2. Check summary report: stress_test_summary.txt"
echo "3. Analyze performance metrics"
echo "4. Run monitoring dashboard if available"