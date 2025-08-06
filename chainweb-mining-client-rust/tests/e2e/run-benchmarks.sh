#!/bin/bash
set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_DIR="$SCRIPT_DIR/docker"
RESULTS_DIR="$SCRIPT_DIR/benchmarks"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BENCHMARK_FILE="$RESULTS_DIR/benchmark_results_${TIMESTAMP}.json"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Default values
HASKELL_IMAGE="${HASKELL_IMAGE:-salamaashoush/chainweb-mining-client:latest}"
RUST_IMAGE="${RUST_IMAGE:-salamaashoush/chainweb-mining-client-rs:latest}"
CHAINWEB_NODE_IMAGE="${CHAINWEB_NODE_IMAGE:-ghcr.io/kadena-io/chainweb-node/ubuntu:latest}"
BENCHMARK_DURATION="${BENCHMARK_DURATION:-300}"  # 5 minutes per benchmark
WORKER_COUNTS="${WORKER_COUNTS:-1 2 4 8}"
THREAD_COUNTS="${THREAD_COUNTS:-1 2 4 8}"

# Functions
log() {
    echo -e "${GREEN}[$(date +%H:%M:%S)]${NC} $1"
}

error() {
    echo -e "${RED}[$(date +%H:%M:%S)] ERROR:${NC} $1" >&2
}

info() {
    echo -e "${BLUE}[$(date +%H:%M:%S)]${NC} $1"
}

cleanup() {
    log "Cleaning up..."
    cd "$DOCKER_DIR"
    docker-compose -f docker-compose.base.yml down -v || true
    docker rm -f $(docker ps -aq --filter "label=benchmark=true") 2>/dev/null || true
}

setup_benchmark_dir() {
    mkdir -p "$RESULTS_DIR"
    echo "{\"benchmark_run\": \"$TIMESTAMP\", \"configuration\": {}, \"results\": []}" > "$BENCHMARK_FILE"
}

measure_resource_usage() {
    local container_name=$1
    local duration=$2
    
    # Start monitoring in background
    local cpu_samples=()
    local mem_samples=()
    
    for i in $(seq 1 $duration); do
        if docker stats --no-stream --format "{{.CPUPerc}} {{.MemUsage}}" "$container_name" 2>/dev/null; then
            local stats=$(docker stats --no-stream --format "{{.CPUPerc}} {{.MemUsage}}" "$container_name" 2>/dev/null)
            local cpu=$(echo "$stats" | awk '{print $1}' | sed 's/%//')
            local mem=$(echo "$stats" | awk '{print $2}' | sed 's/MiB.*//')
            
            cpu_samples+=("$cpu")
            mem_samples+=("$mem")
        fi
        sleep 1
    done &
    
    local monitor_pid=$!
    
    # Return the PID for later collection
    echo "$monitor_pid"
}

calculate_stats() {
    local samples=("$@")
    local count=${#samples[@]}
    
    if [ $count -eq 0 ]; then
        echo "0 0 0"
        return
    fi
    
    # Calculate average
    local sum=0
    for sample in "${samples[@]}"; do
        sum=$(echo "$sum + $sample" | bc -l)
    done
    local avg=$(echo "scale=2; $sum / $count" | bc -l)
    
    # Calculate min/max
    local min=${samples[0]}
    local max=${samples[0]}
    for sample in "${samples[@]}"; do
        if (( $(echo "$sample < $min" | bc -l) )); then
            min=$sample
        fi
        if (( $(echo "$sample > $max" | bc -l) )); then
            max=$sample
        fi
    done
    
    echo "$avg $min $max"
}

benchmark_cpu_worker() {
    log "Benchmarking CPU worker performance..."
    
    for threads in $THREAD_COUNTS; do
        info "Testing with $threads threads..."
        
        # Start Haskell CPU worker
        docker run -d --name cpu-haskell-bench \
            --label benchmark=true \
            --network chainweb-test \
            "$HASKELL_IMAGE" \
            --worker cpu \
            --node chainweb-pow:1848 \
            --public-key f89ef46927f506c70b6a58fd322450a936311dc6ac91f4ec3d8ef949608dbf1f \
            --thread-count "$threads" \
            --log-level info
        
        # Start Rust CPU worker
        docker run -d --name cpu-rust-bench \
            --label benchmark=true \
            --network chainweb-test \
            "$RUST_IMAGE" \
            --worker cpu \
            --node chainweb-pow:1848 \
            --public-key f89ef46927f506c70b6a58fd322450a936311dc6ac91f4ec3d8ef949608dbf1f \
            --thread-count "$threads" \
            --log-level info
        
        # Let them stabilize
        sleep 5
        
        # Collect metrics
        info "Collecting metrics for ${BENCHMARK_DURATION}s..."
        
        # Monitor resource usage
        local haskell_monitor=$(measure_resource_usage "cpu-haskell-bench" "$BENCHMARK_DURATION")
        local rust_monitor=$(measure_resource_usage "cpu-rust-bench" "$BENCHMARK_DURATION")
        
        # Wait for benchmark duration
        sleep "$BENCHMARK_DURATION"
        
        # Stop monitoring
        kill $haskell_monitor $rust_monitor 2>/dev/null || true
        wait $haskell_monitor $rust_monitor 2>/dev/null || true
        
        # Collect results
        local haskell_logs=$(docker logs cpu-haskell-bench 2>&1)
        local rust_logs=$(docker logs cpu-rust-bench 2>&1)
        
        # Count metrics
        local haskell_solutions=$(echo "$haskell_logs" | grep -c "Solution accepted" || echo "0")
        local rust_solutions=$(echo "$rust_logs" | grep -c "Solution accepted" || echo "0")
        local haskell_hashes=$(echo "$haskell_logs" | grep -oE "Hash rate: [0-9.]+ MH/s" | tail -1 | grep -oE "[0-9.]+" || echo "0")
        local rust_hashes=$(echo "$rust_logs" | grep -oE "Hash rate: [0-9.]+ MH/s" | tail -1 | grep -oE "[0-9.]+" || echo "0")
        
        # Save results
        local result=$(jq -n \
            --arg worker "cpu" \
            --arg threads "$threads" \
            --arg duration "$BENCHMARK_DURATION" \
            --arg haskell_solutions "$haskell_solutions" \
            --arg rust_solutions "$rust_solutions" \
            --arg haskell_hashrate "$haskell_hashes" \
            --arg rust_hashrate "$rust_hashes" \
            '{
                worker_type: $worker,
                configuration: {
                    threads: ($threads | tonumber)
                },
                duration_seconds: ($duration | tonumber),
                haskell: {
                    solutions_accepted: ($haskell_solutions | tonumber),
                    hashrate_mhs: ($haskell_hashrate | tonumber),
                    efficiency: (($haskell_solutions | tonumber) / ($threads | tonumber))
                },
                rust: {
                    solutions_accepted: ($rust_solutions | tonumber),
                    hashrate_mhs: ($rust_hashrate | tonumber),
                    efficiency: (($rust_solutions | tonumber) / ($threads | tonumber))
                },
                performance_improvement: {
                    solutions: (if ($haskell_solutions | tonumber) > 0 then ((($rust_solutions | tonumber) - ($haskell_solutions | tonumber)) / ($haskell_solutions | tonumber) * 100) else 0 end),
                    hashrate: (if ($haskell_hashrate | tonumber) > 0 then ((($rust_hashrate | tonumber) - ($haskell_hashrate | tonumber)) / ($haskell_hashrate | tonumber) * 100) else 0 end)
                }
            }')
        
        # Append to results
        jq --argjson result "$result" '.results += [$result]' "$BENCHMARK_FILE" > "$BENCHMARK_FILE.tmp" && mv "$BENCHMARK_FILE.tmp" "$BENCHMARK_FILE"
        
        # Clean up
        docker rm -f cpu-haskell-bench cpu-rust-bench
        
        info "Thread count $threads completed"
    done
}

benchmark_stratum_server() {
    log "Benchmarking Stratum server performance..."
    
    for clients in $WORKER_COUNTS; do
        info "Testing with $clients concurrent clients..."
        
        # Start Haskell Stratum server
        docker run -d --name stratum-haskell-bench \
            --label benchmark=true \
            --network chainweb-test \
            -p 1917:1917 \
            "$HASKELL_IMAGE" \
            --worker stratum \
            --node chainweb-pow:1848 \
            --public-key f89ef46927f506c70b6a58fd322450a936311dc6ac91f4ec3d8ef949608dbf1f \
            --stratum-port 1917 \
            --stratum-difficulty 1 \
            --log-level info
        
        # Start Rust Stratum server
        docker run -d --name stratum-rust-bench \
            --label benchmark=true \
            --network chainweb-test \
            -p 1918:1918 \
            "$RUST_IMAGE" \
            --worker stratum \
            --node chainweb-pow:1848 \
            --public-key f89ef46927f506c70b6a58fd322450a936311dc6ac91f4ec3d8ef949608dbf1f \
            --stratum-port 1918 \
            --stratum-difficulty 1 \
            --log-level info
        
        # Let servers start
        sleep 5
        
        # Start mock clients
        for i in $(seq 1 $clients); do
            docker run -d --name stratum-client-haskell-$i \
                --label benchmark=true \
                --network chainweb-test \
                alpine:latest \
                sh -c "while true; do echo '{\"id\":1,\"method\":\"mining.subscribe\",\"params\":[]}' | nc stratum-haskell-bench 1917; sleep 1; done"
            
            docker run -d --name stratum-client-rust-$i \
                --label benchmark=true \
                --network chainweb-test \
                alpine:latest \
                sh -c "while true; do echo '{\"id\":1,\"method\":\"mining.subscribe\",\"params\":[]}' | nc stratum-rust-bench 1918; sleep 1; done"
        done
        
        # Collect metrics
        info "Collecting metrics for ${BENCHMARK_DURATION}s..."
        sleep "$BENCHMARK_DURATION"
        
        # Collect results
        local haskell_logs=$(docker logs stratum-haskell-bench 2>&1)
        local rust_logs=$(docker logs stratum-rust-bench 2>&1)
        
        # Count connections and shares
        local haskell_connections=$(echo "$haskell_logs" | grep -c "Client connected" || echo "0")
        local rust_connections=$(echo "$rust_logs" | grep -c "Client connected" || echo "0")
        local haskell_shares=$(echo "$haskell_logs" | grep -c "Share accepted" || echo "0")
        local rust_shares=$(echo "$rust_logs" | grep -c "Share accepted" || echo "0")
        
        # Save results
        local result=$(jq -n \
            --arg worker "stratum" \
            --arg clients "$clients" \
            --arg duration "$BENCHMARK_DURATION" \
            --arg haskell_connections "$haskell_connections" \
            --arg rust_connections "$rust_connections" \
            --arg haskell_shares "$haskell_shares" \
            --arg rust_shares "$rust_shares" \
            '{
                worker_type: $worker,
                configuration: {
                    concurrent_clients: ($clients | tonumber)
                },
                duration_seconds: ($duration | tonumber),
                haskell: {
                    connections_handled: ($haskell_connections | tonumber),
                    shares_submitted: ($haskell_shares | tonumber),
                    shares_per_client: (($haskell_shares | tonumber) / ($clients | tonumber))
                },
                rust: {
                    connections_handled: ($rust_connections | tonumber),
                    shares_submitted: ($rust_shares | tonumber),
                    shares_per_client: (($rust_shares | tonumber) / ($clients | tonumber))
                },
                performance_improvement: {
                    connections: (if ($haskell_connections | tonumber) > 0 then ((($rust_connections | tonumber) - ($haskell_connections | tonumber)) / ($haskell_connections | tonumber) * 100) else 0 end),
                    shares: (if ($haskell_shares | tonumber) > 0 then ((($rust_shares | tonumber) - ($haskell_shares | tonumber)) / ($haskell_shares | tonumber) * 100) else 0 end)
                }
            }')
        
        # Append to results
        jq --argjson result "$result" '.results += [$result]' "$BENCHMARK_FILE" > "$BENCHMARK_FILE.tmp" && mv "$BENCHMARK_FILE.tmp" "$BENCHMARK_FILE"
        
        # Clean up
        docker rm -f stratum-haskell-bench stratum-rust-bench
        for i in $(seq 1 $clients); do
            docker rm -f stratum-client-haskell-$i stratum-client-rust-$i
        done
        
        info "Client count $clients completed"
    done
}

generate_benchmark_report() {
    log "Generating benchmark report..."
    
    # Create detailed report
    cat > "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md" <<'EOF'
# Chainweb Mining Client Benchmark Report

## Executive Summary

This report compares the performance of the Haskell and Rust implementations of the chainweb mining client across various worker types and configurations.

EOF
    
    # Add timestamp and configuration
    echo "**Test Date:** $(date)" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    echo "**Duration per test:** ${BENCHMARK_DURATION}s" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    echo "" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    
    # CPU Worker Results
    echo "## CPU Worker Performance" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    echo "" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    echo "| Threads | Haskell Solutions | Rust Solutions | Improvement % | Haskell Hash Rate | Rust Hash Rate |" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    echo "|---------|-------------------|----------------|---------------|-------------------|----------------|" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    
    jq -r '.results[] | select(.worker_type == "cpu") | 
        "| \(.configuration.threads) | \(.haskell.solutions_accepted) | \(.rust.solutions_accepted) | \(.performance_improvement.solutions | tostring | .[0:6])% | \(.haskell.hashrate_mhs) MH/s | \(.rust.hashrate_mhs) MH/s |"' \
        "$BENCHMARK_FILE" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    
    echo "" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    
    # Stratum Server Results
    echo "## Stratum Server Performance" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    echo "" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    echo "| Clients | Haskell Shares | Rust Shares | Improvement % | Haskell Connections | Rust Connections |" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    echo "|---------|----------------|-------------|---------------|---------------------|------------------|" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    
    jq -r '.results[] | select(.worker_type == "stratum") | 
        "| \(.configuration.concurrent_clients) | \(.haskell.shares_submitted) | \(.rust.shares_submitted) | \(.performance_improvement.shares | tostring | .[0:6])% | \(.haskell.connections_handled) | \(.rust.connections_handled) |"' \
        "$BENCHMARK_FILE" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    
    echo "" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    
    # Add summary statistics
    echo "## Summary Statistics" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    echo "" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    
    local avg_cpu_improvement=$(jq '[.results[] | select(.worker_type == "cpu") | .performance_improvement.solutions] | add / length' "$BENCHMARK_FILE")
    local avg_stratum_improvement=$(jq '[.results[] | select(.worker_type == "stratum") | .performance_improvement.shares] | add / length' "$BENCHMARK_FILE" 2>/dev/null || echo "0")
    
    echo "- **Average CPU Worker Improvement:** ${avg_cpu_improvement}%" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    echo "- **Average Stratum Server Improvement:** ${avg_stratum_improvement}%" >> "$RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    
    log "Benchmark report saved to: $RESULTS_DIR/benchmark_report_${TIMESTAMP}.md"
    log "Raw results saved to: $BENCHMARK_FILE"
}

# Main execution
main() {
    log "Starting performance benchmarks"
    log "Configuration:"
    log "  Duration: ${BENCHMARK_DURATION}s per test"
    log "  Thread counts: $THREAD_COUNTS"
    log "  Worker counts: $WORKER_COUNTS"
    
    # Setup
    trap cleanup EXIT
    setup_benchmark_dir
    
    # Start base services
    cd "$DOCKER_DIR"
    docker-compose -f docker-compose.base.yml up -d
    
    # Wait for node to be ready
    log "Waiting for chainweb node..."
    sleep 20
    
    # Run benchmarks
    benchmark_cpu_worker
    benchmark_stratum_server
    
    # Generate report
    generate_benchmark_report
    
    log "Benchmarks completed!"
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --duration)
            BENCHMARK_DURATION="$2"
            shift 2
            ;;
        --threads)
            THREAD_COUNTS="$2"
            shift 2
            ;;
        --workers)
            WORKER_COUNTS="$2"
            shift 2
            ;;
        --help)
            echo "Usage: $0 [options]"
            echo "Options:"
            echo "  --duration SECONDS   Benchmark duration (default: 300)"
            echo "  --threads 'COUNTS'   Thread counts to test (default: '1 2 4 8')"
            echo "  --workers 'COUNTS'   Worker counts to test (default: '1 2 4 8')"
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