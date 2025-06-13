#!/bin/bash
# Test External worker compatibility between Haskell and Rust implementations

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

# Create a mock external miner script
create_mock_miner() {
    local miner_script="/tmp/mock_miner.sh"
    cat > "$miner_script" << 'EOF'
#!/bin/bash
# Mock external miner for testing

# Read work from stdin
read -r work_json

# Extract work bytes (simplified - real miner would parse properly)
echo "Mock miner received work: $work_json" >&2

# Simulate mining for a bit
sleep 2

# Output a mock solution (in real miner, this would be actual nonce)
# Format: nonce as hex string
echo "0000000000000000000000000000000000000000000000000000000000000001"
EOF
    
    chmod +x "$miner_script"
    echo "$miner_script"
}

# Test external worker
test_external_worker() {
    local implementation="$1"
    local binary="$2"
    local mock_miner="$3"
    
    print_step "Testing $implementation external worker..."
    
    local log_file="/tmp/${implementation}_external.log"
    local pid_file="/tmp/${implementation}_external.pid"
    
    # Start the mining client with external worker
    timeout 30s "$binary" \
        --node "localhost:$NODE_PORT" \
        --no-tls \
        --public-key "$PUBLIC_KEY" \
        --account "$ACCOUNT" \
        --worker external \
        --external-command "$mock_miner" \
        --log-level debug \
        > "$log_file" 2>&1 &
    
    local pid=$!
    echo $pid > "$pid_file"
    
    # Give it time to start and attempt mining
    sleep 10
    
    # Check if it's still running
    if kill -0 $pid 2>/dev/null; then
        print_success "$implementation external worker is running"
        
        # Check logs for expected behavior
        if grep -q "external worker" "$log_file"; then
            print_success "$implementation initialized external worker"
        else
            print_warning "$implementation may not have initialized external worker properly"
        fi
        
        # Kill the process
        kill $pid 2>/dev/null || true
        wait $pid 2>/dev/null || true
        
        return 0
    else
        print_error "$implementation external worker failed"
        print_warning "Last 20 lines of log:"
        tail -20 "$log_file"
        return 1
    fi
}

# Compare external worker behavior
compare_external_workers() {
    print_step "Comparing external worker implementations..."
    
    local haskell_log="/tmp/haskell_external.log"
    local rust_log="/tmp/rust_external.log"
    
    # Check if both created external processes
    local haskell_spawned=false
    local rust_spawned=false
    
    if grep -q "Mock miner received work" "$haskell_log" 2>/dev/null; then
        haskell_spawned=true
        print_success "Haskell implementation spawned external miner"
    fi
    
    if grep -q "Mock miner received work" "$rust_log" 2>/dev/null; then
        rust_spawned=true
        print_success "Rust implementation spawned external miner"
    fi
    
    if [ "$haskell_spawned" = true ] && [ "$rust_spawned" = true ]; then
        print_success "Both implementations successfully communicate with external miners"
        return 0
    else
        print_error "External worker implementations differ in behavior"
        return 1
    fi
}

# Main test
main() {
    echo "🔍 External Worker Compatibility Test"
    echo "===================================="
    
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
    
    # Create mock miner
    local mock_miner=$(create_mock_miner)
    print_success "Created mock external miner at $mock_miner"
    
    # Test both implementations
    local haskell_works=false
    local rust_works=false
    
    if [ -f "$HASKELL_BINARY" ]; then
        if test_external_worker "Haskell" "$HASKELL_BINARY" "$mock_miner"; then
            haskell_works=true
        fi
    else
        print_warning "Haskell binary not found, looking for it..."
        HASKELL_BINARY=$(find "$PROJECT_ROOT" -name chainweb-mining-client -type f -executable | grep -v rust | head -1)
        if [ -n "$HASKELL_BINARY" ]; then
            if test_external_worker "Haskell" "$HASKELL_BINARY" "$mock_miner"; then
                haskell_works=true
            fi
        else
            print_error "Could not find Haskell binary"
        fi
    fi
    
    if [ ! -f "$RUST_BINARY" ]; then
        print_warning "Building Rust binary..."
        cd "$PROJECT_ROOT/chainweb-mining-client-rust"
        cargo build --release
    fi
    
    if test_external_worker "Rust" "$RUST_BINARY" "$mock_miner"; then
        rust_works=true
    fi
    
    # Compare behaviors
    if [ "$haskell_works" = true ] && [ "$rust_works" = true ]; then
        compare_external_workers
        print_success "External worker compatibility test PASSED! 🎉"
        exit_code=0
    else
        print_error "External worker compatibility test FAILED"
        exit_code=1
    fi
    
    # Cleanup
    rm -f "$mock_miner"
    cd "$SCRIPT_DIR"
    docker-compose down -v
    
    exit $exit_code
}

# Set up cleanup trap
trap 'cd "$SCRIPT_DIR" && docker-compose down -v; rm -f /tmp/mock_miner.sh' EXIT

# Run main
main