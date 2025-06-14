#!/bin/bash

# Comprehensive benchmark script for chainweb-mining-client-rust
#
# This script runs all available benchmarks and generates reports

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
BENCH_DIR="target/criterion"
QUICK_MODE=false
SAVE_BASELINE=""
COMPARE_BASELINE=""
OUTPUT_DIR=""

# Function to print colored output
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Help function
show_help() {
    cat << EOF
Usage: $0 [OPTIONS]

Run comprehensive benchmarks for chainweb-mining-client-rust

OPTIONS:
    -h, --help              Show this help message
    -q, --quick             Run benchmarks in quick mode (fewer samples)
    -s, --save-baseline NAME Save benchmark results as baseline NAME
    -c, --compare-baseline NAME Compare against baseline NAME
    -o, --output-dir DIR    Save HTML reports to directory DIR
    
EXAMPLES:
    $0                                  # Run all benchmarks
    $0 --quick                          # Quick benchmark run
    $0 --save-baseline main            # Save results as 'main' baseline
    $0 --compare-baseline main         # Compare against 'main' baseline
    $0 --output-dir ./bench-reports    # Save HTML reports to directory

AVAILABLE BENCHMARKS:
    - mining_performance: Core mining operations (hashing, nonce ops, target checking)
    - protocol_performance: Network protocol operations (JSON, binary, retry logic)
    - stratum_performance: Stratum protocol operations (message parsing, job management)
    - config_performance: Configuration parsing and validation operations
EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_help
            exit 0
            ;;
        -q|--quick)
            QUICK_MODE=true
            shift
            ;;
        -s|--save-baseline)
            SAVE_BASELINE="$2"
            shift 2
            ;;
        -c|--compare-baseline)
            COMPARE_BASELINE="$2"
            shift 2
            ;;
        -o|--output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        *)
            print_error "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Check if we're in the right directory
if [[ ! -f "Cargo.toml" ]]; then
    print_error "This script must be run from the chainweb-mining-client-rust directory"
    exit 1
fi

# Check if the bench feature is available
if ! grep -q "bench" Cargo.toml; then
    print_error "Bench feature not found in Cargo.toml"
    exit 1
fi

print_info "Starting comprehensive benchmark suite for chainweb-mining-client-rust"

# Build benchmark arguments
BENCH_ARGS=""
if [[ "$QUICK_MODE" == "true" ]]; then
    BENCH_ARGS="-- --sample-size 10"
    print_info "Running in quick mode (10 samples per benchmark)"
fi

if [[ -n "$SAVE_BASELINE" ]]; then
    BENCH_ARGS="$BENCH_ARGS --save-baseline $SAVE_BASELINE"
    print_info "Will save results as baseline: $SAVE_BASELINE"
fi

if [[ -n "$COMPARE_BASELINE" ]]; then
    BENCH_ARGS="$BENCH_ARGS --baseline $COMPARE_BASELINE"
    print_info "Will compare against baseline: $COMPARE_BASELINE"
fi

# List of benchmarks to run
BENCHMARKS=(
    "mining_performance"
    "protocol_performance" 
    "stratum_performance"
    "config_performance"
)

# Run each benchmark
for bench in "${BENCHMARKS[@]}"; do
    print_info "Running benchmark: $bench"
    
    if cargo bench --features=bench --bench "$bench" $BENCH_ARGS; then
        print_success "Completed benchmark: $bench"
    else
        print_error "Failed benchmark: $bench"
        exit 1
    fi
    
    echo # Add blank line for readability
done

print_success "All benchmarks completed successfully!"

# Generate HTML reports if output directory specified
if [[ -n "$OUTPUT_DIR" ]]; then
    print_info "Generating HTML reports in: $OUTPUT_DIR"
    
    mkdir -p "$OUTPUT_DIR"
    
    # Copy criterion reports if they exist
    if [[ -d "$BENCH_DIR" ]]; then
        cp -r "$BENCH_DIR"/* "$OUTPUT_DIR/"
        print_success "HTML reports generated in: $OUTPUT_DIR"
        print_info "Open $OUTPUT_DIR/index.html to view the reports"
    else
        print_warning "No criterion reports found in $BENCH_DIR"
    fi
fi

# Print summary
print_info "Benchmark Summary:"
echo "=================="
for bench in "${BENCHMARKS[@]}"; do
    echo "✓ $bench"
done

if [[ -n "$SAVE_BASELINE" ]]; then
    echo ""
    print_info "Baseline '$SAVE_BASELINE' saved. Use --compare-baseline $SAVE_BASELINE to compare future runs."
fi

if [[ -n "$COMPARE_BASELINE" ]]; then
    echo ""
    print_info "Comparison complete against baseline '$COMPARE_BASELINE'"
fi

print_success "Benchmark suite completed!"