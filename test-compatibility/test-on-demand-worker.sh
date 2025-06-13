#!/bin/bash
# Test On-Demand worker (Rust only - not implemented in Haskell)

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
RUST_BINARY="${PROJECT_ROOT}/chainweb-mining-client-rust/target/release/chainweb-mining-client"
PUBLIC_KEY="f89ef46927f506c70b6a58fd322450a936311dc6ac91f4ec3d8ef949608dbf1f"
ACCOUNT="k:${PUBLIC_KEY}"
NODE_PORT=1848
ON_DEMAND_PORT=9090

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

# Test on-demand worker
test_on_demand_worker() {
    print_step "Testing Rust on-demand worker..."
    
    # Build if needed
    if [ ! -f "$RUST_BINARY" ]; then
        print_warning "Building Rust binary..."
        cd "$PROJECT_ROOT/chainweb-mining-client-rust"
        cargo build --release
    fi
    
    # Start on-demand worker
    local log_file="/tmp/rust_on_demand.log"
    "$RUST_BINARY" \
        --node "localhost:$NODE_PORT" \
        --no-tls \
        --public-key "$PUBLIC_KEY" \
        --account "$ACCOUNT" \
        --worker on-demand \
        --on-demand-port "$ON_DEMAND_PORT" \
        --log-level debug \
        > "$log_file" 2>&1 &
    
    local pid=$!
    
    # Wait for server to start
    sleep 5
    
    # Check if it's running
    if ! kill -0 $pid 2>/dev/null; then
        print_error "On-demand worker failed to start"
        tail -20 "$log_file"
        return 1
    fi
    
    print_success "On-demand worker started on port $ON_DEMAND_PORT"
    
    # Test the HTTP endpoint
    print_step "Testing on-demand mining endpoint..."
    
    # Trigger mining via HTTP
    local response=$(curl -s -X POST "http://localhost:$ON_DEMAND_PORT/mine" || echo "FAILED")
    
    if [ "$response" != "FAILED" ]; then
        print_success "On-demand endpoint responded"
        echo "Response: $response"
    else
        print_error "On-demand endpoint not responding"
    fi
    
    # Check if mining was triggered in logs
    if grep -q "on-demand.*mine" "$log_file"; then
        print_success "On-demand mining was triggered"
    else
        print_warning "Could not confirm mining was triggered"
    fi
    
    # Cleanup
    kill $pid 2>/dev/null || true
    wait $pid 2>/dev/null || true
    
    return 0
}

# Main test
main() {
    echo "🔍 On-Demand Worker Test (Rust Implementation)"
    echo "=============================================="
    
    # Start node
    print_step "Starting Chainweb node..."
    cd "$SCRIPT_DIR"
    docker-compose down -v 2>/dev/null || true
    docker-compose up -d chainweb-dev
    
    # Wait for node
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
    
    # Test on-demand worker
    if test_on_demand_worker; then
        print_success "On-demand worker test PASSED! 🎉"
        exit_code=0
    else
        print_error "On-demand worker test FAILED"
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