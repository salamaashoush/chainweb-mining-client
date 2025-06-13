#!/bin/bash
# Test Stratum worker compatibility between Haskell and Rust implementations

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
HASKELL_BINARY="${PROJECT_ROOT}/chainweb-mining-client"
RUST_BINARY="${PROJECT_ROOT}/chainweb-mining-client-rust/target/release/chainweb-mining-client"
PUBLIC_KEY="f89ef46927f506c70b6a58fd322450a936311dc6ac91f4ec3d8ef949608dbf1f"
ACCOUNT="k:${PUBLIC_KEY}"
STRATUM_PORT_HASKELL=1917
STRATUM_PORT_RUST=1918
NODE_PORT=1848

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

# Check binaries
check_binaries() {
    print_step "Checking binaries..."
    
    if [ ! -f "$RUST_BINARY" ]; then
        print_warning "Building Rust binary..."
        cd "$PROJECT_ROOT/chainweb-mining-client-rust"
        cargo build --release
    fi
    
    if [ ! -f "$HASKELL_BINARY" ]; then
        print_warning "Looking for Haskell binary..."
        HASKELL_BINARY=$(find "$PROJECT_ROOT" -name chainweb-mining-client -type f -executable | grep -v rust | head -1)
        if [ -z "$HASKELL_BINARY" ]; then
            print_error "Haskell binary not found"
            exit 1
        fi
    fi
    
    print_success "Binaries ready"
}

# Start chainweb node
start_node() {
    print_step "Starting Chainweb node..."
    cd "$SCRIPT_DIR"
    docker-compose down -v 2>/dev/null || true
    docker-compose up -d chainweb-dev
    
    # Wait for node to be healthy
    print_step "Waiting for node to be ready..."
    local max_attempts=30
    local attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        if curl -s "http://localhost:$NODE_PORT/info" >/dev/null 2>&1; then
            print_success "Node is ready"
            break
        fi
        sleep 2
        attempt=$((attempt + 1))
    done
    
    if [ $attempt -eq $max_attempts ]; then
        print_error "Node failed to start"
        docker-compose logs chainweb-dev
        exit 1
    fi
}

# Test Stratum server connectivity
test_stratum_connectivity() {
    local port=$1
    local implementation=$2
    
    print_step "Testing $implementation Stratum connectivity on port $port..."
    
    # Simple telnet test
    if timeout 5 bash -c "echo '{\"id\":1,\"method\":\"mining.authorize\",\"params\":[\"$PUBLIC_KEY\",\"x\"]}' | nc localhost $port | grep -q '\"result\":true'"; then
        print_success "$implementation Stratum server responding correctly"
        return 0
    else
        print_error "$implementation Stratum server not responding correctly"
        return 1
    fi
}

# Run Stratum compatibility test
test_stratum_compatibility() {
    print_step "Starting Stratum compatibility test..."
    
    # Start Haskell Stratum server
    print_step "Starting Haskell Stratum server on port $STRATUM_PORT_HASKELL..."
    local haskell_log="/tmp/haskell_stratum.log"
    "$HASKELL_BINARY" \
        --node "localhost:$NODE_PORT" \
        --no-tls \
        --public-keys "$PUBLIC_KEY" \
        --account "$ACCOUNT" \
        --worker stratum \
        --stratum-port "$STRATUM_PORT_HASKELL" \
        --stratum-interface "0.0.0.0" \
        --log-level debug \
        > "$haskell_log" 2>&1 &
    
    local haskell_pid=$!
    
    # Start Rust Stratum server
    print_step "Starting Rust Stratum server on port $STRATUM_PORT_RUST..."
    local rust_log="/tmp/rust_stratum.log"
    "$RUST_BINARY" \
        --node "localhost:$NODE_PORT" \
        --no-tls \
        --public-key "$PUBLIC_KEY" \
        --account "$ACCOUNT" \
        --worker stratum \
        --stratum-port "$STRATUM_PORT_RUST" \
        --stratum-interface "0.0.0.0" \
        --log-level debug \
        > "$rust_log" 2>&1 &
    
    local rust_pid=$!
    
    # Wait for servers to start
    sleep 5
    
    # Test connectivity
    local haskell_works=false
    local rust_works=false
    
    if test_stratum_connectivity $STRATUM_PORT_HASKELL "Haskell"; then
        haskell_works=true
    fi
    
    if test_stratum_connectivity $STRATUM_PORT_RUST "Rust"; then
        rust_works=true
    fi
    
    # Run the expect script tests
    if [ "$haskell_works" = true ]; then
        print_step "Running expect script test on Haskell implementation..."
        local expect_script="$PROJECT_ROOT/scripts/stratum.expect"
        if [ -f "$expect_script" ]; then
            # Modify expect script for our port
            sed "s/localhost 1917/localhost $STRATUM_PORT_HASKELL/g" "$expect_script" > /tmp/test_haskell.expect
            if timeout 30 expect /tmp/test_haskell.expect; then
                print_success "Haskell Stratum passes expect script test"
            else
                print_error "Haskell Stratum fails expect script test"
            fi
        fi
    fi
    
    if [ "$rust_works" = true ]; then
        print_step "Running expect script test on Rust implementation..."
        local expect_script="$PROJECT_ROOT/scripts/stratum.expect"
        if [ -f "$expect_script" ]; then
            # Modify expect script for our port and public key
            sed -e "s/localhost 1917/localhost $STRATUM_PORT_RUST/g" \
                -e "s/cc13b2e497f90b5d9d13ba4217ea578cd21e258a194a4fe6f43f87f02eae71be/$PUBLIC_KEY/g" \
                "$expect_script" > /tmp/test_rust.expect
            if timeout 30 expect /tmp/test_rust.expect; then
                print_success "Rust Stratum passes expect script test"
            else
                print_error "Rust Stratum fails expect script test"
            fi
        fi
    fi
    
    # Compare protocol responses
    if [ "$haskell_works" = true ] && [ "$rust_works" = true ]; then
        print_step "Comparing protocol responses..."
        
        # Test mining.authorize
        local haskell_auth=$(echo '{"id":1,"method":"mining.authorize","params":["'$PUBLIC_KEY'","x"]}' | nc -w 2 localhost $STRATUM_PORT_HASKELL | head -1)
        local rust_auth=$(echo '{"id":1,"method":"mining.authorize","params":["'$PUBLIC_KEY'","x"]}' | nc -w 2 localhost $STRATUM_PORT_RUST | head -1)
        
        print_step "Haskell auth response: $haskell_auth"
        print_step "Rust auth response: $rust_auth"
        
        # Both should contain success
        if echo "$haskell_auth" | grep -q '"result":true' && echo "$rust_auth" | grep -q '"result":true'; then
            print_success "Both implementations handle authorization correctly"
        else
            print_error "Authorization responses differ"
        fi
    fi
    
    # Cleanup
    kill $haskell_pid 2>/dev/null || true
    kill $rust_pid 2>/dev/null || true
    wait $haskell_pid 2>/dev/null || true
    wait $rust_pid 2>/dev/null || true
    
    # Show logs if there were failures
    if [ "$haskell_works" = false ]; then
        print_warning "Haskell Stratum logs:"
        tail -50 "$haskell_log"
    fi
    
    if [ "$rust_works" = false ]; then
        print_warning "Rust Stratum logs:"
        tail -50 "$rust_log"
    fi
    
    # Final result
    if [ "$haskell_works" = true ] && [ "$rust_works" = true ]; then
        print_success "Stratum compatibility test PASSED"
        return 0
    else
        print_error "Stratum compatibility test FAILED"
        return 1
    fi
}

# Main function
main() {
    echo "🔍 Stratum Worker Compatibility Test"
    echo "===================================="
    
    check_binaries
    start_node
    
    # Give node extra time
    sleep 5
    
    # Run Stratum test
    if test_stratum_compatibility; then
        print_success "All Stratum tests passed! 🎉"
        exit_code=0
    else
        print_error "Some Stratum tests failed"
        exit_code=1
    fi
    
    # Cleanup
    cd "$SCRIPT_DIR"
    docker-compose down -v
    
    exit $exit_code
}

# Set up cleanup trap
trap 'cd "$SCRIPT_DIR" && docker-compose down -v' EXIT

# Run main
main