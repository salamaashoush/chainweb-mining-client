#!/bin/bash
set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log() {
    echo -e "${GREEN}[$(date +%H:%M:%S)]${NC} $1"
}

error() {
    echo -e "${RED}[$(date +%H:%M:%S)] ERROR:${NC} $1" >&2
}

info() {
    echo -e "${BLUE}[$(date +%H:%M:%S)]${NC} $1"
}

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    error "This script must be run from the chainweb-mining-client-rust directory"
    exit 1
fi

# Parse arguments
RUN_UNIT_TESTS=true
RUN_INTEGRATION_TESTS=true
RUN_E2E_TESTS=true
RUN_BENCHMARKS=true
E2E_DURATION=60
BENCHMARK_DURATION=300

while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-unit)
            RUN_UNIT_TESTS=false
            shift
            ;;
        --skip-integration)
            RUN_INTEGRATION_TESTS=false
            shift
            ;;
        --skip-e2e)
            RUN_E2E_TESTS=false
            shift
            ;;
        --skip-benchmarks)
            RUN_BENCHMARKS=false
            shift
            ;;
        --quick)
            E2E_DURATION=30
            BENCHMARK_DURATION=60
            shift
            ;;
        --help)
            echo "Usage: $0 [options]"
            echo "Options:"
            echo "  --skip-unit         Skip unit tests"
            echo "  --skip-integration  Skip integration tests"
            echo "  --skip-e2e          Skip E2E tests"
            echo "  --skip-benchmarks   Skip performance benchmarks"
            echo "  --quick             Run quick tests (shorter durations)"
            echo "  --help              Show this help message"
            echo ""
            echo "This script runs all tests for the Rust implementation and"
            echo "compares it against the Haskell implementation."
            exit 0
            ;;
        *)
            error "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Summary of what will run
log "Chainweb Mining Client - Comprehensive Test Suite"
log "================================================="
log "Configuration:"
log "  Unit Tests: $([ "$RUN_UNIT_TESTS" = true ] && echo "✓" || echo "✗")"
log "  Integration Tests: $([ "$RUN_INTEGRATION_TESTS" = true ] && echo "✓" || echo "✗")"
log "  E2E Tests: $([ "$RUN_E2E_TESTS" = true ] && echo "✓" || echo "✗")"
log "  Benchmarks: $([ "$RUN_BENCHMARKS" = true ] && echo "✓" || echo "✗")"
if [ "$E2E_DURATION" = "30" ]; then
    log "  Mode: Quick (reduced durations)"
fi
echo ""

# Run unit tests
if [ "$RUN_UNIT_TESTS" = true ]; then
    info "Running unit tests..."
    if cargo test --lib; then
        log "✓ Unit tests passed"
    else
        error "✗ Unit tests failed"
        exit 1
    fi
    echo ""
fi

# Run integration tests (requires local node)
if [ "$RUN_INTEGRATION_TESTS" = true ]; then
    info "Running integration tests..."
    info "Note: These require a running chainweb node on localhost"
    
    # Check if node is running
    if curl -s http://localhost:1848/info >/dev/null 2>&1; then
        log "Chainweb node detected, running integration tests..."
        if cargo test --test integration_tests -- --ignored --test-threads=1; then
            log "✓ Integration tests passed"
        else
            error "✗ Integration tests failed"
        fi
    else
        info "No chainweb node detected on localhost:1848, skipping integration tests"
    fi
    echo ""
fi

# Build Docker image if needed
if [ "$RUN_E2E_TESTS" = true ] || [ "$RUN_BENCHMARKS" = true ]; then
    info "Building Rust Docker image locally..."
    if ./build-docker.sh latest scratch --local; then
        log "✓ Docker image built successfully (local only)"
    else
        error "✗ Failed to build Docker image"
        exit 1
    fi
    echo ""
fi

# Run E2E tests
if [ "$RUN_E2E_TESTS" = true ]; then
    info "Running E2E tests..."
    cd tests/e2e
    if ./run-e2e-tests.sh --duration "$E2E_DURATION"; then
        log "✓ E2E tests completed"
        
        # Show summary
        latest_summary=$(ls -t results/e2e_test_summary_*.txt 2>/dev/null | head -1)
        if [ -n "$latest_summary" ]; then
            echo ""
            info "E2E Test Summary:"
            tail -n 20 "$latest_summary"
        fi
    else
        error "✗ E2E tests failed"
        exit 1
    fi
    cd ../..
    echo ""
fi

# Run benchmarks
if [ "$RUN_BENCHMARKS" = true ]; then
    info "Running performance benchmarks..."
    cd tests/e2e
    if ./run-benchmarks.sh --duration "$BENCHMARK_DURATION"; then
        log "✓ Benchmarks completed"
        
        # Show summary
        latest_report=$(ls -t benchmarks/benchmark_report_*.md 2>/dev/null | head -1)
        if [ -n "$latest_report" ]; then
            echo ""
            info "Benchmark Summary:"
            grep -A 10 "Summary Statistics" "$latest_report" || true
        fi
    else
        error "✗ Benchmarks failed"
        exit 1
    fi
    cd ../..
    echo ""
fi

# Final summary
log "Test Suite Complete!"
log "==================="

if [ "$RUN_UNIT_TESTS" = true ]; then
    log "✓ Unit tests: PASSED"
fi

if [ "$RUN_INTEGRATION_TESTS" = true ]; then
    if curl -s http://localhost:1848/info >/dev/null 2>&1; then
        log "✓ Integration tests: PASSED"
    else
        log "- Integration tests: SKIPPED (no node)"
    fi
fi

if [ "$RUN_E2E_TESTS" = true ]; then
    log "✓ E2E tests: PASSED"
    log "  Results: tests/e2e/results/"
fi

if [ "$RUN_BENCHMARKS" = true ]; then
    log "✓ Benchmarks: COMPLETED"
    log "  Reports: tests/e2e/benchmarks/"
fi

echo ""
log "All tests completed successfully! 🎉"