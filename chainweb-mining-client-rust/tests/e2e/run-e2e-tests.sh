#!/bin/bash
set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_DIR="$SCRIPT_DIR/docker"
RESULTS_DIR="$SCRIPT_DIR/results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULT_FILE="$RESULTS_DIR/e2e_test_results_${TIMESTAMP}.json"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Default values
HASKELL_IMAGE="${HASKELL_IMAGE:-salamaashoush/chainweb-mining-client:latest}"
RUST_IMAGE="${RUST_IMAGE:-salamaashoush/chainweb-mining-client-rs:latest}"
CHAINWEB_NODE_IMAGE="${CHAINWEB_NODE_IMAGE:-ghcr.io/kadena-io/chainweb-node/ubuntu:latest}"
TEST_DURATION="${TEST_DURATION:-60}"
WORKER_TYPES="${WORKER_TYPES:-cpu stratum simulation constant-delay on-demand external}"

# Functions
log() {
    echo -e "${GREEN}[$(date +%H:%M:%S)]${NC} $1"
}

error() {
    echo -e "${RED}[$(date +%H:%M:%S)] ERROR:${NC} $1" >&2
}

warning() {
    echo -e "${YELLOW}[$(date +%H:%M:%S)] WARNING:${NC} $1" >&2
}

cleanup() {
    log "Cleaning up..."
    cd "$DOCKER_DIR"
    docker-compose -f docker-compose.base.yml down -v || true
    for worker in $WORKER_TYPES; do
        docker-compose -f docker-compose.base.yml -f docker-compose.$worker.yml down -v || true
    done
}

check_prerequisites() {
    log "Checking prerequisites..."
    
    missing_deps=()
    command -v docker >/dev/null 2>&1 || missing_deps+=("docker")
    command -v docker-compose >/dev/null 2>&1 || missing_deps+=("docker-compose")
    command -v jq >/dev/null 2>&1 || missing_deps+=("jq")
    
    if [ ${#missing_deps[@]} -ne 0 ]; then
        error "Missing required tools: ${missing_deps[*]}"
        exit 1
    fi
    
    # Check Docker daemon
    if ! docker info >/dev/null 2>&1; then
        error "Docker daemon is not running"
        exit 1
    fi
    
    log "All prerequisites satisfied"
}

setup_results_dir() {
    mkdir -p "$RESULTS_DIR"
    echo "{\"test_run\": \"$TIMESTAMP\", \"results\": []}" > "$RESULT_FILE"
}

pull_images() {
    log "Pulling Docker images..."
    docker pull "$HASKELL_IMAGE" || warning "Failed to pull Haskell image"
    docker pull "$RUST_IMAGE" || warning "Failed to pull Rust image"
    docker pull "$CHAINWEB_NODE_IMAGE" || warning "Failed to pull Chainweb node image"
}

start_base_services() {
    log "Starting base services..."
    cd "$DOCKER_DIR"
    docker-compose -f docker-compose.base.yml up -d
    
    # Wait for nodes to be healthy
    log "Waiting for chainweb nodes to be ready..."
    max_attempts=90
    attempt=0
    while [ $attempt -lt $max_attempts ]; do
        if docker-compose -f docker-compose.base.yml ps | grep -q "healthy"; then
            log "Chainweb nodes are ready"
            break
        fi
        sleep 2
        attempt=$((attempt + 1))
        if [ $((attempt % 15)) -eq 0 ]; then
            echo -n " ${attempt}s "
        else
            echo -n "."
        fi
    done
    echo
    
    if [ $attempt -eq $max_attempts ]; then
        error "Chainweb nodes failed to become healthy"
        return 1
    fi
}

test_worker() {
    local worker_type=$1
    local use_pow_node=$2
    
    log "Testing $worker_type worker..."
    
    # Determine which node to use
    local node_name="chainweb-pow"
    if [ "$use_pow_node" = "false" ]; then
        node_name="chainweb-no-pow"
    fi
    
    # Start worker services
    cd "$DOCKER_DIR"
    docker-compose -f docker-compose.base.yml -f docker-compose.$worker_type.yml up -d
    
    # Give workers time to start
    sleep 10
    
    # Collect metrics
    log "Collecting metrics for $worker_type worker..."
    
    # Run for test duration
    sleep "$TEST_DURATION"
    
    # Collect logs and metrics
    local haskell_logs=$(docker logs mining-client-$worker_type-haskell 2>&1 || echo "No logs")
    local rust_logs=$(docker logs mining-client-$worker_type-rust 2>&1 || echo "No logs")
    
    # Count solutions
    local haskell_solutions=$(echo "$haskell_logs" | grep -c "Solution accepted" | tr -d '\n' || echo "0")
    local rust_solutions=$(echo "$rust_logs" | grep -c "Solution accepted" | tr -d '\n' || echo "0")
    
    # Count errors
    local haskell_errors=$(echo "$haskell_logs" | grep -c "ERROR" | tr -d '\n' || echo "0")
    local rust_errors=$(echo "$rust_logs" | grep -c "ERROR" | tr -d '\n' || echo "0")
    
    # Save results
    local result=$(jq -n \
        --arg worker "$worker_type" \
        --arg haskell_solutions "$haskell_solutions" \
        --arg rust_solutions "$rust_solutions" \
        --arg haskell_errors "$haskell_errors" \
        --arg rust_errors "$rust_errors" \
        --arg duration "$TEST_DURATION" \
        '{
            worker_type: $worker,
            duration_seconds: ($duration | tonumber),
            haskell: {
                solutions_accepted: ($haskell_solutions | tonumber),
                errors: ($haskell_errors | tonumber),
                solutions_per_minute: (($haskell_solutions | tonumber) * 60 / ($duration | tonumber))
            },
            rust: {
                solutions_accepted: ($rust_solutions | tonumber),
                errors: ($rust_errors | tonumber),
                solutions_per_minute: (($rust_solutions | tonumber) * 60 / ($duration | tonumber))
            }
        }')
    
    # Append to results file
    jq --argjson result "$result" '.results += [$result]' "$RESULT_FILE" > "$RESULT_FILE.tmp" && mv "$RESULT_FILE.tmp" "$RESULT_FILE"
    
    # Log summary
    log "Results for $worker_type worker:"
    log "  Haskell: $haskell_solutions solutions, $haskell_errors errors"
    log "  Rust: $rust_solutions solutions, $rust_errors errors"
    
    # Stop worker services
    docker-compose -f docker-compose.base.yml -f docker-compose.$worker_type.yml down
}

generate_report() {
    log "Generating test report..."
    
    # Calculate summary
    local total_tests=$(jq '.results | length' "$RESULT_FILE")
    local failed_tests=$(jq '[.results[] | select(.haskell.errors > 0 or .rust.errors > 0)] | length' "$RESULT_FILE")
    
    # Generate summary report
    cat > "$RESULTS_DIR/e2e_test_summary_${TIMESTAMP}.txt" <<EOF
E2E Test Summary
================
Timestamp: $TIMESTAMP
Total Tests: $total_tests
Failed Tests: $failed_tests
Test Duration: ${TEST_DURATION}s per worker

Worker Type Results:
--------------------
EOF
    
    # Add detailed results
    jq -r '.results[] | "
\(.worker_type) Worker:
  Haskell:
    - Solutions: \(.haskell.solutions_accepted)
    - Errors: \(.haskell.errors)
    - Rate: \(.haskell.solutions_per_minute | tostring | .[0:5]) solutions/min
  Rust:
    - Solutions: \(.rust.solutions_accepted)
    - Errors: \(.rust.errors)
    - Rate: \(.rust.solutions_per_minute | tostring | .[0:5]) solutions/min
  Performance Ratio (Rust/Haskell): \(if .haskell.solutions_accepted > 0 then (.rust.solutions_accepted / .haskell.solutions_accepted * 100 | tostring | .[0:5]) else "N/A" end)%
"' "$RESULT_FILE" >> "$RESULTS_DIR/e2e_test_summary_${TIMESTAMP}.txt"
    
    log "Test report saved to: $RESULTS_DIR/e2e_test_summary_${TIMESTAMP}.txt"
    log "Detailed results saved to: $RESULT_FILE"
}

# Main execution
main() {
    log "Starting E2E tests for chainweb-mining-client"
    log "Configuration:"
    log "  Haskell Image: $HASKELL_IMAGE"
    log "  Rust Image: $RUST_IMAGE"
    log "  Test Duration: ${TEST_DURATION}s per worker"
    log "  Worker Types: $WORKER_TYPES"
    
    # Setup
    trap cleanup EXIT
    check_prerequisites
    setup_results_dir
    pull_images
    
    # Start base services
    if ! start_base_services; then
        error "Failed to start base services"
        exit 1
    fi
    
    # Test each worker type
    for worker in $WORKER_TYPES; do
        case $worker in
            cpu|stratum|external)
                test_worker "$worker" "true"
                ;;
            simulation|constant-delay|on-demand)
                test_worker "$worker" "false"
                ;;
            *)
                warning "Unknown worker type: $worker"
                ;;
        esac
    done
    
    # Generate report
    generate_report
    
    log "E2E tests completed successfully!"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --duration)
            TEST_DURATION="$2"
            shift 2
            ;;
        --workers)
            WORKER_TYPES="$2"
            shift 2
            ;;
        --haskell-image)
            HASKELL_IMAGE="$2"
            shift 2
            ;;
        --rust-image)
            RUST_IMAGE="$2"
            shift 2
            ;;
        --help)
            echo "Usage: $0 [options]"
            echo "Options:"
            echo "  --duration SECONDS      Test duration per worker (default: 60)"
            echo "  --workers 'TYPES'       Space-separated worker types (default: all)"
            echo "  --haskell-image IMAGE   Haskell implementation Docker image"
            echo "  --rust-image IMAGE      Rust implementation Docker image"
            echo "  --help                  Show this help message"
            exit 0
            ;;
        *)
            error "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Run main
main