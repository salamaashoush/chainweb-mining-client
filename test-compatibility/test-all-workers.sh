#!/bin/bash
# Comprehensive compatibility test script for all worker types
# Tests both Haskell and Rust implementations

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"
# get it with cabal
HASKELL_BINARY=$(cabal list-bin chainweb-mining-client)
RUST_BINARY="${PROJECT_ROOT}/chainweb-mining-client-rust/target/release/chainweb-mining-client"
PUBLIC_KEY="f89ef46927f506c70b6a58fd322450a936311dc6ac91f4ec3d8ef949608dbf1f"
ACCOUNT="k:${PUBLIC_KEY}"

# Test results tracking
declare -A test_results

# Functions for colored output
print_step() {
    echo -e "${BLUE}🔍 $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

# Check if required binaries exist
check_binaries() {
    print_step "Checking binaries..."
    
    if [ ! -f "$HASKELL_BINARY" ]; then
        print_error "Haskell binary not found at $HASKELL_BINARY"
        print_warning "Building Haskell binary..."
        cd "$PROJECT_ROOT"
        cabal build
        HASKELL_BINARY=$(find . -name chainweb-mining-client -type f -executable | grep -v rust | head -1)
        if [ -z "$HASKELL_BINARY" ]; then
            print_error "Failed to find Haskell binary after build"
            exit 1
        fi
        HASKELL_BINARY="$PROJECT_ROOT/$HASKELL_BINARY"
    fi
    
    if [ ! -f "$RUST_BINARY" ]; then
        print_warning "Rust binary not found, building..."
        cd "$PROJECT_ROOT/chainweb-mining-client-rust"
        cargo build --release
    fi
    
    print_success "Binaries ready"
}

# Start chainweb nodes
start_nodes() {
    print_step "Starting Chainweb nodes..."
    cd "$SCRIPT_DIR"
    docker-compose down -v 2>/dev/null || true
    docker-compose up -d
    
    # Wait for nodes to be ready by checking API endpoint
    print_step "Waiting for nodes to be ready..."
    local max_attempts=30
    local attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        if curl -s http://localhost:1848/info >/dev/null 2>&1; then
            print_success "Development node is ready"
            break
        fi
        sleep 2
        attempt=$((attempt + 1))
    done
    
    if [ $attempt -eq $max_attempts ]; then
        print_error "Nodes failed to start"
        docker-compose logs
        exit 1
    fi
}

# Stop chainweb nodes
stop_nodes() {
    print_step "Stopping Chainweb nodes..."
    cd "$SCRIPT_DIR"
    docker-compose down -v
}

# Test a specific worker with both implementations
test_worker() {
    local worker_type="$1"
    local node_port="$2"
    local haskell_extra_args="$3"
    local rust_extra_args="$4"
    local test_name="$5"
    
    print_step "Testing $test_name"
    
    # Test with Haskell implementation
    print_step "Testing Haskell implementation..."
    local haskell_log="/tmp/haskell_${worker_type}_${node_port}.log"
    timeout 30s "$HASKELL_BINARY" \
        --node "localhost:$node_port" \
        --no-tls \
        --public-key "$PUBLIC_KEY" \
        --account "$ACCOUNT" \
        --worker "$worker_type" \
        --log-level debug \
        $haskell_extra_args \
        > "$haskell_log" 2>&1 &
    
    local haskell_pid=$!
    
    # Give it time to start and mine
    sleep 10
    
    # Check if it's still running
    if kill -0 $haskell_pid 2>/dev/null; then
        print_success "Haskell $worker_type worker running"
        kill $haskell_pid 2>/dev/null || true
        wait $haskell_pid 2>/dev/null || true
        test_results["haskell_${worker_type}_${node_port}"]="PASS"
    else
        print_error "Haskell $worker_type worker failed"
        cat "$haskell_log"
        test_results["haskell_${worker_type}_${node_port}"]="FAIL"
    fi
    
    # Test with Rust implementation
    print_step "Testing Rust implementation..."
    local rust_log="/tmp/rust_${worker_type}_${node_port}.log"
    timeout 30s "$RUST_BINARY" \
        --node "localhost:$node_port" \
        --no-tls \
        --public-key "$PUBLIC_KEY" \
        --account "$ACCOUNT" \
        --worker "$worker_type" \
        --log-level debug \
        $rust_extra_args \
        > "$rust_log" 2>&1 &
    
    local rust_pid=$!
    
    # Give it time to start and mine
    sleep 10
    
    # Check if it's still running
    if kill -0 $rust_pid 2>/dev/null; then
        print_success "Rust $worker_type worker running"
        kill $rust_pid 2>/dev/null || true
        wait $rust_pid 2>/dev/null || true
        test_results["rust_${worker_type}_${node_port}"]="PASS"
    else
        print_error "Rust $worker_type worker failed"
        cat "$rust_log"
        test_results["rust_${worker_type}_${node_port}"]="FAIL"
    fi
    
    # Compare logs for compatibility
    if [ "${test_results["haskell_${worker_type}_${node_port}"]}" = "PASS" ] && \
       [ "${test_results["rust_${worker_type}_${node_port}"]}" = "PASS" ]; then
        print_success "Both implementations work for $worker_type on port $node_port"
    else
        print_error "Compatibility issue detected for $worker_type on port $node_port"
    fi
    
    echo ""
}

# Test CPU worker
test_cpu_worker() {
    # Haskell uses --thread-count, Rust uses --thread-count
    test_worker "cpu" "1848" "--thread-count 2" "--thread-count 2" "CPU Worker (Dev Node)"
}

# Test Simulation worker
test_simulation_worker() {
    # Haskell uses --hash-rate, Rust uses --hash-rate
    test_worker "simulation" "1848" "--hash-rate 1000000" "--hash-rate 1000000" "Simulation Worker (Dev Node)"
}

# Test Constant Delay worker
test_constant_delay_worker() {
    # Haskell uses --constant-delay-block-time, Rust uses --constant-delay-block-time
    test_worker "constant-delay" "1848" "--constant-delay-block-time 5" "--constant-delay-block-time 5" "Constant Delay Worker (Dev Node)"
}

# Main test execution
main() {
    echo "🔍 Chainweb Mining Client Compatibility Test Suite"
    echo "=================================================="
    
    # Setup
    check_binaries
    start_nodes
    
    # Give nodes extra time to fully initialize
    sleep 5
    
    # Run tests
    test_cpu_worker
    test_simulation_worker
    test_constant_delay_worker
    
    # Print summary
    echo ""
    echo "Test Summary"
    echo "============"
    
    local total_tests=0
    local passed_tests=0
    
    for test in "${!test_results[@]}"; do
        total_tests=$((total_tests + 1))
        if [ "${test_results[$test]}" = "PASS" ]; then
            print_success "$test: PASS"
            passed_tests=$((passed_tests + 1))
        else
            print_error "$test: FAIL"
        fi
    done
    
    echo ""
    echo "Total: $passed_tests/$total_tests tests passed"
    
    # Cleanup
    stop_nodes
    
    if [ $passed_tests -eq $total_tests ]; then
        print_success "All tests passed! 🎉"
        exit 0
    else
        print_error "Some tests failed"
        exit 1
    fi
}

# Handle script arguments
case "${1:-}" in
    --keep-nodes)
        # Don't stop nodes after tests
        trap - EXIT
        main
        ;;
    --help|-h)
        echo "Usage: $0 [OPTIONS]"
        echo ""
        echo "Options:"
        echo "  --keep-nodes    Don't stop Docker nodes after tests"
        echo "  --help, -h      Show this help message"
        exit 0
        ;;
    "")
        # Set up cleanup trap
        trap stop_nodes EXIT
        main
        ;;
    *)
        print_error "Unknown argument: $1"
        echo "Use --help for usage information"
        exit 1
        ;;
esac
