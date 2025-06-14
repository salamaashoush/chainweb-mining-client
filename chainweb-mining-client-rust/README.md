# Chainweb Mining Client (Rust)

A high-performance, async Rust implementation of the Chainweb mining client with support for CPU mining, GPU mining, and Stratum protocol.

## Features

- **Async/await architecture** using Tokio for maximum performance
- **Multiple worker types**:
  - CPU mining with multi-threading support
  - External worker support for GPU miners
  - Stratum server for ASIC miners (in progress)
- **Real-time updates** via Server-Sent Events (SSE)
- **Comprehensive configuration** via CLI and TOML config files
- **Production-ready logging** with structured tracing
- **Type-safe** implementation with zero unsafe code
- **100% compatible** with the Haskell implementation

## Installation

### Prerequisites

- Rust 1.70 or higher
- Cargo

### Automatic Dependency Installation

The project includes a cross-platform script that automatically installs all required dependencies:

```bash
# Automatic setup for Arch Linux, Ubuntu, Fedora, and openSUSE
just dev-setup

# Or run the script directly
./scripts/install-deps.sh
```

**Supported Distributions:**
- Arch Linux / Manjaro
- Ubuntu / Debian / Linux Mint / Pop!_OS / elementary OS
- Fedora / RHEL / CentOS / Rocky Linux / AlmaLinux
- openSUSE Leap / Tumbleweed / SLES

**Installed Tools:**
- System packages: expect, telnet, netcat, docker, git, build tools
- Rust tools: cargo-machete, typos-cli, cargo-audit, cargo-llvm-cov
- Command runner: just
- Container platform: Docker with proper permissions

### Building from source

```bash
git clone https://github.com/kadena-io/chainweb-mining-client.git
cd chainweb-mining-client/chainweb-mining-client-rust

# Quick start with all quality checks
just check
cargo build --release

# Or using traditional cargo commands
cargo build --release
```

The binary will be available at `target/release/chainweb-mining-client`.

## Usage

### Basic CPU mining

```bash
chainweb-mining-client \
  --node api.chainweb.com \
  --chain-id 0 \
  --account k:your-account \
  --public-key your-public-key
```

### Using a configuration file

```bash
chainweb-mining-client --config mining-config.toml
```

### Configuration file example

```toml
[node]
url = "api.chainweb.com"
use_tls = true
timeout_secs = 30
chain_id = 0

[mining]
account = "k:your-account"
public_key = "your-public-key"
update_interval_secs = 5

[worker]
type = "cpu"
threads = 0  # 0 = use all cores
batch_size = 100000

[logging]
level = "info"
format = "plain"
```

### External GPU worker

```toml
[worker]
type = "external"
command = "/path/to/gpu-miner"
args = ["--gpu", "0"]
timeout_secs = 60
```

### Command-line options

```
OPTIONS:
    -c, --config <FILE>              Configuration file path
    -n, --node <NODE>                Node URL [env: CHAINWEB_NODE_URL]
    -i, --chain-id <CHAIN_ID>        Chain ID to mine on [env: CHAINWEB_CHAIN_ID]
    -a, --account <ACCOUNT>          Miner account name [env: CHAINWEB_ACCOUNT]
    -k, --public-key <PUBLIC_KEY>    Miner public key [env: CHAINWEB_PUBLIC_KEY]
    -w, --worker <WORKER>            Worker type [default: cpu]
    -t, --threads <THREADS>          Number of threads (CPU worker) [default: 0]
    -l, --log-level <LOG_LEVEL>      Log level [default: info]
        --stratum-port <PORT>        Stratum server port [default: 3333]
        --external-command <PATH>    External worker command
    -h, --help                       Print help
    -V, --version                    Print version
```

## Architecture

### Core Components

- **`core/`** - Core types (Work, Target, Nonce, ChainId)
- **`protocol/`** - Chainweb node communication
- **`workers/`** - Mining worker implementations
- **`config/`** - Configuration management
- **`utils/`** - Utility functions

### Worker Types

1. **CPU Worker** - Multi-threaded CPU mining using Rayon
2. **External Worker** - Interface for GPU and custom miners
3. **Stratum Server** - Mining pool protocol for ASICs

### Mining Flow

1. Connect to Chainweb node and get node info
2. Subscribe to work updates via SSE
3. Fetch initial work
4. Start mining with selected worker
5. Submit solutions when found
6. Update work when notified

## Development

### Quick Start

```bash
# Install system dependencies and development tools
just dev-setup

# Verify all tools are installed correctly
just dev-verify

# Run all quality checks (recommended before committing)
just check

# Run CI pipeline locally
just ci

# Quick development checks
just dev-check
```

### Available Commands

```bash
# Quality Checks
just fmt          # Format code with rustfmt
just lint         # Run clippy linter with strict settings
just typos        # Check for spelling errors
just unused-deps  # Check for unused dependencies
just audit        # Run security audit

# Testing and Building
just test         # Run all tests
just build        # Build debug version
just build-release # Build optimized release
just bench        # Run basic benchmarks
just bench-all    # Run comprehensive benchmark suite
just bench-quick  # Run quick benchmarks (fewer samples)
just bench-save main           # Save benchmark baseline
just bench-compare main        # Compare against baseline
just bench-report ./reports    # Generate HTML reports

# Docker Support
just docker-build latest scratch     # Build scratch-based image
just docker-build latest distroless  # Build distroless image
just docker-build-all v1.0.0        # Build both variants
just docker-test latest             # Test image functionality

# Complete Workflows
just release v1.0.0  # Full release: check + test + build + docker

# Stratum Protocol Testing
just test-stratum         # Full integration test with expect script
just test-stratum-unit    # Unit tests for protocol compatibility
```

### Using Cargo Make

This project uses `cargo-make` for task automation, providing a shell-script-free development experience with better cross-platform support and maintainability.

#### Quick Start

```bash
# Install cargo-make (one-time setup)
cargo install cargo-make

# Show available tasks
cargo make

# Complete development setup
cargo make dev-setup         # Install tools and dependencies
```

#### Daily Development

```bash
cargo make dev               # Format, lint, build, and test
cargo make build             # Build release version
cargo make test              # Run all tests
cargo make clean             # Clean artifacts and logs
```

#### Testing & Validation

```bash
# Comprehensive testing
cargo make stress-test       # Run full stress test suite
cargo make ci               # Run CI checks

# Individual worker testing
cargo make stress-test-cpu              # CPU worker stress test
cargo make stress-test-constant-delay   # Constant-delay worker test
cargo make stress-test-stratum          # Stratum server test
cargo make stress-test-simulation       # Simulation worker test
```

#### Node Management

```bash
cargo make start-node-dev    # Start development node (POW disabled)
cargo make start-node-prod   # Start production-like node
cargo make node-status       # Check node status
cargo make node-logs         # View node logs
cargo make stop-node         # Stop running node
```

#### Advanced Features

```bash
# Docker support
cargo make docker-build      # Build Docker images
cargo make docker-test       # Test Docker images

# Benchmarking
cargo make bench-all         # Run comprehensive benchmarks
cargo make bench-quick       # Quick benchmarks

# Compatibility testing
cargo make test-compat       # Test against Haskell implementation
```

**Key Benefits:**
- ✅ **Cross-platform**: Works on Windows, macOS, and Linux
- ✅ **No shell scripts**: Pure Rust-based task automation
- ✅ **Self-documenting**: All tasks include descriptions
- ✅ **Dependency management**: Automatic tool installation
- ✅ **Environment support**: Customizable via environment variables

See [CARGO_MAKE_GUIDE.md](CARGO_MAKE_GUIDE.md) for complete documentation.

### Code Quality Standards

This project maintains zero compiler warnings and follows strict quality standards:

- **100% Warning-Free**: All compiler warnings have been eliminated
- **Formatted**: Code is automatically formatted with `rustfmt`
- **Linted**: Uses `clippy` with `-D warnings` (treat warnings as errors)
- **Spell-Checked**: Uses `typos-cli` to catch spelling errors
- **Dependency-Clean**: No unused dependencies via `cargo-machete`
- **Security-Audited**: Regular vulnerability scans with `cargo audit`

### Continuous Integration

The project uses GitHub Actions for comprehensive CI/CD:

- **Multi-Platform Testing**: Linux, Windows, macOS
- **Multi-Architecture Builds**: x86_64, aarch64
- **Docker Images**: Both scratch and distroless variants
- **Security Scanning**: Automated dependency vulnerability checks
- **Code Coverage**: Comprehensive test coverage reporting
- **Automated Releases**: Binary artifacts and Docker images on git tags

### Node Requirements

For mining to work, the Chainweb node must have mining endpoints enabled:
- Production nodes: Mining is enabled by default
- Development nodes: Must be started with `DISABLE_POW_VALIDATION=1` environment variable
- The node must expose the following endpoints:
  - `/chainweb/0.0/{version}/mining/work`
  - `/chainweb/0.0/{version}/mining/updates`
  - `/chainweb/0.0/{version}/mining/solved`

### Stratum Protocol Testing

The project includes comprehensive testing for Stratum protocol compatibility:

```bash
# Protocol unit tests (9 test cases validating Haskell compatibility)
cargo test --test stratum_compatibility

# Full integration test with expect script (requires expect and telnet)
./scripts/test-stratum.sh

# Available via justfile (run from chainweb-mining-client-rust directory)
cd chainweb-mining-client-rust
just test-stratum-unit      # Unit tests only
just test-stratum           # Full integration test (requires node on port 8080)
just test-stratum 1848      # Test with node on custom port
just test-stratum-no-node   # Test without requiring a running node
```

**Test Coverage:**
- **Message Format Compatibility**: Validates JSON-RPC message structures
- **Protocol Flow**: Tests mining.authorize and mining.subscribe sequences  
- **Haskell Compatibility**: Uses the original `stratum.expect` script
- **Error Handling**: Validates error response formats
- **Performance**: Multi-connection stress testing

### Traditional Cargo Commands

```bash
# Run all tests
cargo test

# Run with coverage (requires cargo-llvm-cov)
cargo llvm-cov --html

# Run benchmarks
cargo bench --features bench

# Check without building
cargo check

# Format code
cargo fmt

# Run clippy
cargo clippy -- -D warnings
```

### Documentation

```bash
# Generate and open documentation
cargo doc --open
```

### Code style

The project uses standard Rust formatting:

```bash
cargo fmt
cargo clippy
```

## Performance

The Rust implementation provides several performance advantages:

- **Zero-copy parsing** of work headers
- **Lock-free concurrent hashmap** for session management
- **SIMD-optimized Blake2s** hashing
- **Efficient memory layout** with no allocations in hot paths
- **Parallel nonce checking** with work-stealing scheduler

Benchmarks show 2-3x performance improvement over the Haskell implementation for CPU mining.

### Benchmarks

The project includes comprehensive benchmarks measuring performance across all critical components:

```bash
# Run all benchmarks
./scripts/bench.sh

# Quick benchmark run (fewer samples)
./scripts/bench.sh --quick

# Save results as baseline for comparison
./scripts/bench.sh --save-baseline main

# Compare against previous baseline
./scripts/bench.sh --compare-baseline main

# Generate HTML reports
./scripts/bench.sh --output-dir ./reports
```

**Benchmark Categories:**

- **Mining Performance**: Core operations (hashing: ~270ns, nonce ops: <1ns, target checking)
- **Protocol Performance**: Network operations (JSON serialization, binary encoding, retry logic)
- **Stratum Performance**: Mining pool protocol (message parsing, job management, share validation)
- **Configuration Performance**: Config parsing and validation (YAML, JSON, TOML)

For detailed benchmark documentation, see [BENCHMARKS.md](BENCHMARKS.md).

## Compatibility

This implementation is 100% compatible with the original Haskell client:
- Same work format (286 bytes)
- Same Blake2s-256 hashing algorithm
- Same API endpoints and protocols
- Same configuration options

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Write tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## License

BSD-3-Clause License (same as the original Haskell implementation)

## Acknowledgments

Based on the original Haskell implementation by the Kadena team.