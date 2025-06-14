# Cargo Make Guide

This project uses `cargo-make` as the primary task runner for all development, testing, and deployment workflows. All bash script functionality has been integrated into cargo-make tasks for better cross-platform support and maintainability.

## Installation

First, install cargo-make:

```bash
cargo install cargo-make
```

## Quick Start

```bash
# Show all available tasks
cargo make

# Build the project
cargo make build

# Run tests
cargo make test

# Start a development node and run stress tests
cargo make start-node-dev
cargo make stress-test
```

## Core Development Tasks

### Building and Testing

```bash
cargo make build          # Build in release mode
cargo make build-debug    # Build in debug mode
cargo make test           # Run all tests
cargo make test-unit      # Run unit tests only
cargo make test-integration # Run integration tests only
```

### Code Quality

```bash
cargo make dev            # Format, lint, build, and test
cargo make format         # Format code
cargo make format-check   # Check formatting
cargo make lint           # Run clippy
cargo make clean          # Clean artifacts and logs
cargo make clean-logs     # Clean only log files (keeps build artifacts)
```

### Development Setup

```bash
cargo make dev-setup      # Install tools and dependencies
cargo make dev-verify     # Verify all tools are installed
cargo make dev-check      # Quick development checks
```

## Node Management

Manage chainweb nodes for testing without external scripts:

```bash
# Start nodes
cargo make start-node-dev    # Start development node (POW disabled)
cargo make start-node-prod   # Start production-like node

# Monitor nodes
cargo make node-status       # Check node status
cargo make node-logs         # Show node logs

# Stop nodes
cargo make stop-node         # Stop the running node
```

## Stress Testing

Run comprehensive stress tests for all worker types:

```bash
# Full stress test suite
cargo make stress-test

# Individual worker stress tests
cargo make stress-test-cpu              # Test CPU worker
cargo make stress-test-stratum          # Test Stratum server
cargo make stress-test-constant-delay   # Test constant-delay worker
cargo make stress-test-simulation       # Test simulation worker
```

### Customizing Stress Tests

Use environment variables to customize test behavior:

```bash
# Set test duration (default: 60 seconds)
TEST_DURATION=120 cargo make stress-test-cpu

# Set worker count (default: 4)
WORKER_COUNT=8 cargo make stress-test-cpu

# Use custom node endpoint
NODE_ENDPOINT=http://localhost:8080 cargo make stress-test

# Set custom public key
PUBLIC_KEY=your_key_here cargo make stress-test
```

## Stratum Protocol Testing

Test Stratum protocol compatibility:

```bash
cargo make test-stratum           # Full integration test with expect script
cargo make test-stratum-unit      # Unit tests only
```

## Benchmarking

Run performance benchmarks:

```bash
cargo make bench-all         # Run comprehensive benchmark suite
cargo make bench-quick       # Quick benchmarks (fewer samples)
cargo make bench-save main   # Save benchmark baseline
cargo make bench-compare main # Compare against baseline
```

## Docker Support

Build and test Docker images:

```bash
cargo make docker-build latest scratch     # Build scratch-based image
cargo make docker-build latest distroless  # Build distroless image
cargo make docker-build-all v1.0.0        # Build both variants
cargo make docker-test latest             # Test image functionality
cargo make docker-clean                   # Clean Docker cache
```

## Compatibility Testing

Test compatibility with the Haskell implementation:

```bash
cargo make test-compat                    # Run all compatibility tests
cargo make test-compat-cpu                # Test CPU worker
cargo make test-compat-stratum            # Test Stratum server
cargo make test-compat-constant-delay     # Test constant-delay worker
```

## CI/CD Tasks

Tasks designed for continuous integration:

```bash
cargo make ci        # Run CI checks (format, lint, build, test)
cargo make ci-full   # Run full CI including stress tests
```

## Complete Workflows

High-level workflows that combine multiple tasks:

```bash
cargo make release v1.0.0  # Complete release: check + test + build + docker
```

## Environment Variables

Customize task behavior with environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log level for Rust applications |
| `NODE_ENDPOINT` | `http://localhost:1848` | Chainweb node endpoint |
| `PUBLIC_KEY` | `f89ef46927f5...` | Default miner public key |
| `ACCOUNT` | `k:f89ef46927f5...` | Default miner account |
| `TEST_DURATION` | `60` | Test duration in seconds |
| `WORKER_COUNT` | `4` | Number of workers for stress tests |

## Task Dependencies

Cargo-make automatically handles task dependencies:

- **Build dependencies**: All test tasks automatically build the project first
- **Node dependencies**: Tasks that need a node will start one if not running
- **POW dependencies**: Constant-delay tests ensure the node has POW disabled

## Shell Script Migration

This project has migrated away from bash scripts to provide better cross-platform support:

| Original Bash Script | Cargo Make Task | Benefits |
|---------------------|-----------------|----------|
| `scripts/e2e-stress-test.sh` | `cargo make stress-test` | Cross-platform, better error handling |
| `scripts/install-deps.sh` | `cargo make dev-setup` | Automatic platform detection |
| `scripts/bench.sh` | `cargo make bench-all` | Integrated benchmark management |
| `test-compatibility/start-chainweb-node.sh` | `cargo make start-node-dev` | Unified node management |
| `test-compatibility/test-all-workers.sh` | `cargo make test-compat` | Better dependency tracking |

### Key Benefits of Migration:
- **Cross-platform**: Works on Windows without WSL
- **Dependency management**: Automatic tool installation and verification
- **Self-documenting**: All tasks include descriptions
- **Error handling**: Better error messages and recovery
- **Type safety**: Parameter validation

## Advanced Usage

### Task Information

```bash
# List all available tasks
cargo make --list

# Show task description and details
cargo make --describe stress-test

# Print task definition without running
cargo make --print-steps stress-test
```

### Parallel Execution

Some tasks can be run in parallel for faster execution:

```bash
# Run multiple tasks in sequence
cargo make format lint test

# For parallel execution, use multiple terminals or external tools
```

### Custom Configuration

Create a local `Makefile.toml` to override or extend tasks for your specific needs:

```toml
[env]
# Custom environment variables
MY_CUSTOM_VAR = "value"

[tasks.my-custom-task]
description = "My custom development task"
script = '''
echo "Running custom task with ${MY_CUSTOM_VAR}"
'''
```

## Log File Management

Stress tests and benchmarks generate log files for debugging and analysis:

### Generated Log Files

- `stress_test_*.log` - Output from stress test workers
- `benchmark_*.log` - Benchmark execution logs  
- `node_*.log` - Chainweb node logs
- `docker_*.log` - Docker container logs

### Cleanup Commands

```bash
# Clean all artifacts and logs
cargo make clean

# Clean only log files (keeps build artifacts)
cargo make clean-logs

# Manual cleanup
rm -f stress_test_*.log
```

### Git Integration

Log files are automatically ignored by git via `.gitignore` patterns:

```gitignore
stress_test_*.log
benchmark_*.log
node_*.log
chainweb_*.log
# ... and more
```

## Troubleshooting

### Common Issues

1. **cargo-make not found**: Install with `cargo install cargo-make`
2. **Node connection failed**: Ensure Docker is running and node is started
3. **Tool not found**: Run `cargo make dev-setup` to install dependencies
4. **Tests failing**: Check that required services are running
5. **Too many log files**: Run `cargo make clean-logs` to clean up

### Debugging Tasks

```bash
# Run with verbose output
cargo make --verbose stress-test

# Check what would be executed
cargo make --dry-run stress-test

# Show full task information
cargo make --describe stress-test
```

### Getting Help

- **Show all tasks**: `cargo make` or `cargo make --list`
- **Task details**: `cargo make --describe <task-name>`
- **Cargo-make help**: `cargo make --help`
- **This guide**: Comprehensive documentation for all project tasks

## Best Practices

1. **Use `cargo make dev` for daily development**: Runs format, lint, build, and test
2. **Always run setup after updates**: `cargo make dev-setup` after system changes
3. **Use environment variables for customization**: Set `TEST_DURATION`, `WORKER_COUNT`, etc.
4. **Clean up after stress testing**: `cargo make clean` removes logs and artifacts
5. **Verify tools periodically**: `cargo make dev-verify` ensures all tools are working

## Integration Examples

### CI/CD Pipeline (GitHub Actions)

```yaml
name: CI
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Install cargo-make
        run: cargo install cargo-make
      - name: Run CI checks
        run: cargo make ci
      - name: Run stress tests
        run: cargo make stress-test
```

### Development Workflow

```bash
# Daily development cycle
cargo make dev-setup      # One-time setup
cargo make dev           # Regular development checks
cargo make stress-test-cpu # Test specific functionality
cargo make clean         # Clean up when done
```

### Release Workflow

```bash
# Prepare for release
cargo make ci            # Full CI checks
cargo make test-compat   # Compatibility tests
cargo make release v1.0.0 # Complete release workflow
```

This guide provides comprehensive coverage of all cargo-make tasks available in this project. For the latest task list, always run `cargo make` to see current options.