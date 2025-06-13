# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Development Commands

### Haskell Implementation

**Using Cabal:**
```bash
# Build the project
cabal update
cabal build

# Run tests
cabal test

# Run the mining client
cabal run chainweb-mining-client -- --help

# Generate a new key pair
cabal run chainweb-mining-client -- --generate-key
```

**Using Nix:**
```bash
# Build the project
nix build .

# Enter development shell with all dependencies
nix develop .

# Build all recursive outputs
nix build .#recursive.allDerivations
```

**Docker:**
```bash
# Build multi-architecture Docker image
./build.sh <docker-username> [tag]

# Run from Docker
docker run ghcr.io/kadena-io/chainweb-mining-client:latest --help
```

### Rust Implementation

The project includes a Rust rewrite in the `chainweb-mining-client-rust/` directory:

**Quick Start:**
```bash
cd chainweb-mining-client-rust

# Build release version
cargo build --release

# Run all quality checks (recommended before committing)
just check

# Run tests
cargo test

# Run CI checks (includes tests and release build)
just ci
```

**Development Tools:**
```bash
# Install development tools
just dev-setup

# Format code
just fmt

# Run clippy linter
just lint

# Check for typos
just typos

# Check for unused dependencies
just unused-deps

# Quick development check
just dev-check

# Alternative: use the shell script
./check.sh help

# Test Stratum protocol compatibility
just test-stratum         # Full integration test (requires expect/telnet)
just test-stratum-unit    # Unit tests only
```

**Available Checks:**
- **Compiler Warnings**: All warnings have been fixed
- **Code Formatting**: Uses `rustfmt` for consistent styling
- **Linting**: Uses `clippy` with strict settings (`-D warnings`)
- **Typo Detection**: Uses `typos-cli` to catch spelling errors
- **Unused Dependencies**: Uses `cargo-machete` to detect unused deps
- **Security Audit**: Uses `cargo audit` for dependency vulnerabilities

**Docker Support:**
```bash
# Build Docker images
just docker-build latest scratch     # Build with scratch base
just docker-build latest distroless  # Build with distroless base
just docker-build-all v1.0.0        # Build both variants

# Test Docker image
just docker-test latest

# Clean Docker cache
just docker-clean

# Complete release workflow
just release v1.0.0  # Runs checks, tests, builds, and creates Docker images
```

**Continuous Integration:**
- **GitHub Actions**: Automated CI/CD pipeline for testing, building, and releasing
- **Multi-platform builds**: Linux (x86_64, aarch64), Windows, macOS
- **Multi-architecture Docker**: amd64 and arm64 support
- **Automated releases**: Binary artifacts and Docker images published on tags
- **Security scanning**: Automated dependency vulnerability checks
- **Code coverage**: Comprehensive test coverage reporting

## Architecture Overview

### Core Components

**Mining Client Architecture:**
- The client connects to a Kadena Chainweb node's mining API to obtain work
- Supports multiple worker types via a plugin architecture
- Uses async/concurrent processing for efficiency
- All mining rewards go to the configured Pact account

**Worker Types:**
1. **Stratum Server** (`Worker.POW.Stratum.Server`): Serves work to ASIC miners via Stratum protocol
2. **CPU Worker** (`Worker.POW.CPU`): Multi-threaded CPU mining using Blake2s-256
3. **External Worker** (`Worker.External`): Interfaces with external programs (e.g., GPU miners)
4. **Simulated Worker** (`Worker.SimulatedMiner`): For testing with configurable hash rates
5. **Constant Delay** (`Worker.ConstantDelay`): Emits blocks at fixed intervals (dev mode)
6. **On Demand** (`Worker.OnDemand`): HTTP server that mines blocks on request (dev mode)

**Key Modules:**
- `JsonRpc.hs`: Handles JSON-RPC communication with chainweb nodes
- `Target.hs`: Mining difficulty and target calculations
- `Worker.hs`: Abstract worker interface that all worker types implement
- `Worker.POW.Stratum.Protocol`: Stratum protocol message definitions
- `Logger.hs`: Structured logging with configurable levels

**Configuration System:**
- Uses `configuration-tools` library for YAML/JSON config files
- Configuration cascade: files → command-line args
- Supports remote config files via HTTP/HTTPS

**Stratum Server Details:**
- Binds to configurable interface/port (default: *:1917)
- Supports multiple concurrent ASIC connections
- Configurable difficulty: fixed or dynamic (block-based)
- Job emission rate control via `--stratum-rate`
- Multiple worker threads for redundancy

## Testing Approach

**Test Suite:**
- Located in `test/` directory
- Uses `sydtest` framework
- Test data files in `test/data/`
- Run with: `cabal test`

**Stratum Testing:**
- Expect script at `scripts/stratum.expect` for protocol testing
- Tests authentication and subscription flows
- **Rust Implementation**: Comprehensive Stratum test suite with expect script compatibility
- **Unit Tests**: Protocol format validation and Haskell compatibility tests
- **Integration Tests**: Full server testing with mock clients

## Key Implementation Notes

1. **Blake2s-256 Mining**: All mining uses the Blake2s-256 hash algorithm
2. **Work Format**: Uses Kadena's specific work header binary format
3. **Account Format**: Mining accounts use `k:` prefix by default
4. **TLS Support**: Can connect to nodes via HTTPS with `--tls` flag
5. **Development Modes**: Non-PoW modes require `DISABLE_POW_VALIDATION=1` on the node

## Compatibility Testing

The project includes comprehensive compatibility tests between Haskell and Rust implementations:

### Running Compatibility Tests

```bash
# From the Rust directory
cd chainweb-mining-client-rust

# Run all compatibility tests
just test-compat-all

# Test specific components
just test-compat-stratum      # Stratum protocol compatibility
just test-compat-workers      # All worker types
just test-compat-external     # External worker functionality
just test-compat-on-demand    # On-demand worker (Rust only)
```

### Manual Testing with Docker

```bash
# Start a development chainweb node
cd test-compatibility
./start-chainweb-node.sh dev

# Start a production-like node
./start-chainweb-node.sh prod

# The node will be available at http://localhost:1848
```

The compatibility tests use Docker to run actual Chainweb nodes and test both implementations against them.