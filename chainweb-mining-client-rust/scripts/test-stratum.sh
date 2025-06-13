#!/bin/bash
# Comprehensive Stratum server testing script for Rust implementation

set -e

# Get the directory where this script is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_DIR="$( cd "$SCRIPT_DIR/.." && pwd )"

# Change to project directory to ensure paths work correctly
cd "$PROJECT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
STRATUM_PORT=1917
STRATUM_HOST="localhost"
NODE_HOST="localhost"
NODE_PORT=8080
PUBLIC_KEY="f89ef46927f506c70b6a58fd322450a936311dc6ac91f4ec3d8ef949608dbf1f"
TEST_TIMEOUT=30
BINARY_PATH="./target/debug/chainweb-mining-client"
EXPECT_SCRIPT="../scripts/stratum.expect"

# Function to print colored output
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

# Check if required tools are installed
check_dependencies() {
    local missing_deps=false
    
    if ! command -v expect >/dev/null 2>&1; then
        print_error "expect is not installed. Install with: sudo apt-get install expect"
        missing_deps=true
    fi
    
    if ! command -v telnet >/dev/null 2>&1; then
        print_error "telnet is not installed. Install with: sudo apt-get install telnet"
        missing_deps=true
    fi
    
    if ! command -v nc >/dev/null 2>&1; then
        print_error "netcat is not installed. Install with: sudo apt-get install netcat"
        missing_deps=true
    fi
    
    if [ "$missing_deps" = true ]; then
        exit 1
    fi
}

# Check if port is available
check_port() {
    if nc -z $STRATUM_HOST $STRATUM_PORT 2>/dev/null; then
        print_error "Port $STRATUM_PORT is already in use"
        exit 1
    fi
}

# Check if Chainweb node is available
check_node() {
    print_step "Checking Chainweb node at $NODE_HOST:$NODE_PORT..."
    
    if curl -s -f "http://$NODE_HOST:$NODE_PORT/info" >/dev/null 2>&1; then
        print_success "Chainweb node is accessible"
    else
        print_error "Cannot connect to Chainweb node at $NODE_HOST:$NODE_PORT"
        print_warning "Please ensure a Chainweb node is running on port $NODE_PORT"
        print_warning "You can start a devnet node with: chainweb-node --dev"
        exit 1
    fi
}

# Wait for server to start
wait_for_server() {
    local max_attempts=30
    local attempt=0
    
    print_step "Waiting for Stratum server to start on port $STRATUM_PORT..."
    
    while [ $attempt -lt $max_attempts ]; do
        if nc -z $STRATUM_HOST $STRATUM_PORT 2>/dev/null; then
            print_success "Stratum server is ready"
            return 0
        fi
        sleep 1
        attempt=$((attempt + 1))
    done
    
    print_error "Stratum server failed to start within $max_attempts seconds"
    return 1
}

# Start the Rust mining client with Stratum server
start_stratum_server() {
    print_step "Starting Rust Stratum server..."
    
    # Build the binary if it doesn't exist
    if [ ! -f "$BINARY_PATH" ]; then
        print_step "Building Rust binary..."
        cargo build
    fi
    
    # Start the server in the background
    $BINARY_PATH \
        --node "$NODE_HOST:$NODE_PORT" \
        --no-tls \
        --public-key "$PUBLIC_KEY" \
        --account "k:$PUBLIC_KEY" \
        --worker stratum \
        --stratum-port $STRATUM_PORT \
        --stratum-interface "0.0.0.0" \
        --log-level debug > "$PROJECT_DIR/stratum_server.log" 2>&1 &
    
    SERVER_PID=$!
    echo $SERVER_PID > "$PROJECT_DIR/stratum_server.pid"
    
    print_step "Started Stratum server with PID: $SERVER_PID"
}

# Stop the Stratum server
stop_stratum_server() {
    if [ -f "$PROJECT_DIR/stratum_server.pid" ]; then
        local pid=$(cat "$PROJECT_DIR/stratum_server.pid")
        print_step "Stopping Stratum server (PID: $pid)..."
        kill $pid 2>/dev/null || true
        rm -f "$PROJECT_DIR/stratum_server.pid"
        sleep 2
    fi
}

# Run the original expect script
run_expect_test() {
    print_step "Running original Haskell expect test..."
    
    if [ ! -f "$EXPECT_SCRIPT" ]; then
        print_error "Expect script not found at $EXPECT_SCRIPT"
        return 1
    fi
    
    # Copy and modify the expect script for our port and public key
    local temp_script="/tmp/stratum_test.expect"
    sed -e "s/localhost 1917/localhost $STRATUM_PORT/g" \
        -e "s/cc13b2e497f90b5d9d13ba4217ea578cd21e258a194a4fe6f43f87f02eae71be/$PUBLIC_KEY/g" \
        "$EXPECT_SCRIPT" > "$temp_script"
    chmod +x "$temp_script"
    
    if expect "$temp_script"; then
        print_success "Expect test passed"
        rm -f "$temp_script"
        return 0
    else
        print_error "Expect test failed"
        rm -f "$temp_script"
        return 1
    fi
}

# Test basic connectivity
test_connectivity() {
    print_step "Testing basic TCP connectivity..."
    
    if echo "test" | nc -w 5 $STRATUM_HOST $STRATUM_PORT >/dev/null 2>&1; then
        print_success "TCP connectivity test passed"
        return 0
    else
        print_error "TCP connectivity test failed"
        return 1
    fi
}

# Test JSON-RPC protocol manually
test_json_rpc() {
    print_step "Testing JSON-RPC protocol..."
    
    # Test mining.subscribe
    local subscribe_response
    subscribe_response=$(echo '{"id": 1, "method": "mining.subscribe", "params": ["test-miner"]}' | nc -w 5 $STRATUM_HOST $STRATUM_PORT)
    
    if echo "$subscribe_response" | grep -q '"id":1'; then
        print_success "mining.subscribe test passed"
    else
        print_error "mining.subscribe test failed"
        echo "Response: $subscribe_response"
        return 1
    fi
    
    # Test mining.authorize
    local auth_response
    auth_response=$(echo '{"id": 2, "method": "mining.authorize", "params": ["test-user", "test-pass"]}' | nc -w 5 $STRATUM_HOST $STRATUM_PORT)
    
    if echo "$auth_response" | grep -q '"id":2'; then
        print_success "mining.authorize test passed"
    else
        print_error "mining.authorize test failed"
        echo "Response: $auth_response"
        return 1
    fi
    
    return 0
}

# Performance test - multiple connections
test_performance() {
    print_step "Testing multiple concurrent connections..."
    
    local num_connections=10
    local pids=()
    
    for i in $(seq 1 $num_connections); do
        (
            echo '{"id": '$i', "method": "mining.subscribe", "params": ["perf-test-'$i'"]}' | nc -w 5 $STRATUM_HOST $STRATUM_PORT >/dev/null 2>&1
        ) &
        pids+=($!)
    done
    
    # Wait for all connections to complete
    local failed=0
    for pid in "${pids[@]}"; do
        if ! wait $pid; then
            failed=$((failed + 1))
        fi
    done
    
    if [ $failed -eq 0 ]; then
        print_success "Performance test passed: $num_connections concurrent connections"
        return 0
    else
        print_error "Performance test failed: $failed/$num_connections connections failed"
        return 1
    fi
}

# Show server logs if test fails
show_logs() {
    if [ -f "$PROJECT_DIR/stratum_server.log" ]; then
        print_step "Server logs:"
        echo "----------------------------------------"
        tail -50 "$PROJECT_DIR/stratum_server.log"
        echo "----------------------------------------"
    fi
}

# Cleanup function
cleanup() {
    print_step "Cleaning up..."
    stop_stratum_server
    rm -f "$PROJECT_DIR/stratum_server.log" "$PROJECT_DIR/stratum_test.expect"
}

# Main test function
main() {
    echo "🔍 Stratum Server Test Suite for Rust Implementation"
    echo "====================================================="
    
    # Set up cleanup trap
    trap cleanup EXIT
    
    # Check dependencies
    check_dependencies
    
    # Check if Chainweb node is available (unless --skip-node-check is passed)
    if [ "${SKIP_NODE_CHECK:-false}" != "true" ]; then
        check_node
    else
        print_warning "Skipping node check (--skip-node-check)"
    fi
    
    # Check if port is available
    check_port
    
    # Start the server
    start_stratum_server
    
    # Wait for server to be ready
    if ! wait_for_server; then
        show_logs
        exit 1
    fi
    
    # Run tests
    local test_results=()
    
    # Basic connectivity test
    if test_connectivity; then
        test_results+=("✅ Connectivity")
    else
        test_results+=("❌ Connectivity")
        show_logs
    fi
    
    # JSON-RPC protocol test
    if test_json_rpc; then
        test_results+=("✅ JSON-RPC Protocol")
    else
        test_results+=("❌ JSON-RPC Protocol")
        show_logs
    fi
    
    # Original expect test
    if run_expect_test; then
        test_results+=("✅ Haskell Compatibility")
    else
        test_results+=("❌ Haskell Compatibility")
        show_logs
    fi
    
    # Performance test
    if test_performance; then
        test_results+=("✅ Performance")
    else
        test_results+=("❌ Performance")
        show_logs
    fi
    
    # Show results
    echo ""
    echo "📊 Test Results:"
    echo "================"
    for result in "${test_results[@]}"; do
        echo "  $result"
    done
    
    # Check if all tests passed
    local failed_count=$(echo "${test_results[@]}" | grep -o "❌" | wc -l)
    
    if [ $failed_count -eq 0 ]; then
        echo ""
        print_success "All Stratum tests passed! 🎉"
        exit 0
    else
        echo ""
        print_error "$failed_count test(s) failed"
        exit 1
    fi
}

# Parse command line arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --skip-node-check)
                SKIP_NODE_CHECK=true
                shift
                ;;
            --node-port)
                NODE_PORT="$2"
                shift 2
                ;;
            --node-host)
                NODE_HOST="$2"
                shift 2
                ;;
            --stratum-port)
                STRATUM_PORT="$2"
                shift 2
                ;;
            --help|-h)
                echo "Usage: $0 [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  --skip-node-check     Skip checking if Chainweb node is available"
                echo "  --node-host HOST      Chainweb node host (default: localhost)"
                echo "  --node-port PORT      Chainweb node port (default: 8080)"
                echo "  --stratum-port PORT   Stratum server port (default: 1917)"
                echo "  --help, -h            Show this help message"
                exit 0
                ;;
            *)
                print_error "Unknown argument: $1"
                echo "Use --help for usage information"
                exit 1
                ;;
        esac
    done
}

# Run main function if script is executed directly
if [ "${BASH_SOURCE[0]}" == "${0}" ]; then
    parse_args "$@"
    main
fi
