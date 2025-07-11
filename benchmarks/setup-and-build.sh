#!/bin/bash
# Automated setup and build script for benchmarking

set -e  # Exit on error

echo "==================================="
echo "Chainweb Mining Client Build Setup"
echo "==================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to print status
print_status() {
    echo -e "${GREEN}[✓]${NC} $1"
}

print_error() {
    echo -e "${RED}[✗]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[!]${NC} $1"
}

# Check prerequisites
echo -e "\n${YELLOW}Checking prerequisites...${NC}"

if ! command_exists git; then
    print_error "Git not found. Please install git first."
    exit 1
fi

if ! command_exists cabal; then
    print_error "Cabal not found. Please install Haskell first:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://get-ghcup.haskell.org | sh"
    exit 1
else
    print_status "Cabal found: $(cabal --version | head -1)"
fi

if ! command_exists cargo; then
    print_error "Cargo not found. Please install Rust first:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
else
    print_status "Cargo found: $(cargo --version)"
fi

# Detect current directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo -e "\n${YELLOW}Project root: $PROJECT_ROOT${NC}"

# Build Haskell implementation
echo -e "\n${YELLOW}Building Haskell implementation...${NC}"
cd "$PROJECT_ROOT"

if [ -f "cabal.project" ]; then
    print_status "Found cabal.project"
    
    # Update cabal package list
    echo "Updating cabal packages..."
    cabal update
    
    # Build with optimizations
    echo "Building Haskell implementation (this may take 15-30 minutes on first run)..."
    if cabal build --enable-tests --enable-benchmarks; then
        print_status "Haskell build successful"
        
        # Find and display the executable path
        HASKELL_EXE=$(find dist-newstyle -name chainweb-mining-client -type f -executable | grep -v ".git" | head -1)
        if [ -n "$HASKELL_EXE" ]; then
            print_status "Haskell executable: $HASKELL_EXE"
            echo "export HASKELL_MINING_CLIENT=\"$PROJECT_ROOT/$HASKELL_EXE\"" > "$SCRIPT_DIR/paths.env"
        else
            print_warning "Could not find Haskell executable"
        fi
    else
        print_error "Haskell build failed"
        exit 1
    fi
else
    print_error "No cabal.project found in $PROJECT_ROOT"
    exit 1
fi

# Build Rust implementation
echo -e "\n${YELLOW}Building Rust implementation...${NC}"
cd "$PROJECT_ROOT/chainweb-mining-client-rust"

if [ -f "Cargo.toml" ]; then
    print_status "Found Cargo.toml"
    
    # Build with optimizations
    echo "Building Rust implementation (this may take 5-10 minutes on first run)..."
    if cargo build --release; then
        print_status "Rust build successful"
        
        RUST_EXE="target/release/chainweb-mining-client"
        if [ -f "$RUST_EXE" ]; then
            print_status "Rust executable: $RUST_EXE"
            echo "export RUST_MINING_CLIENT=\"$PROJECT_ROOT/chainweb-mining-client-rust/$RUST_EXE\"" >> "$SCRIPT_DIR/paths.env"
        else
            print_warning "Could not find Rust executable"
        fi
    else
        print_error "Rust build failed"
        exit 1
    fi
else
    print_error "No Cargo.toml found in chainweb-mining-client-rust"
    exit 1
fi

# Verify both builds
echo -e "\n${YELLOW}Verifying builds...${NC}"
cd "$PROJECT_ROOT"

if [ -n "$HASKELL_EXE" ] && [ -f "$HASKELL_EXE" ]; then
    if $HASKELL_EXE --help >/dev/null 2>&1; then
        print_status "Haskell executable verified"
    else
        print_error "Haskell executable failed to run"
    fi
fi

if [ -f "chainweb-mining-client-rust/$RUST_EXE" ]; then
    if chainweb-mining-client-rust/$RUST_EXE --help >/dev/null 2>&1; then
        print_status "Rust executable verified"
    else
        print_error "Rust executable failed to run"
    fi
fi

# Create benchmark directories
echo -e "\n${YELLOW}Setting up benchmark directories...${NC}"
mkdir -p "$SCRIPT_DIR"/{results,logs,scripts}
print_status "Created benchmark directories"

# Generate example config
cat > "$SCRIPT_DIR/benchmark-config.env" << EOF
# Benchmark Configuration
# Generated on $(date)

# Test account (generate your own with --generate-key)
export MINER_ACCOUNT="k:f90ef36c9a3da8fbb0cb8d5bf421c15862eeed62b042818762492f2488963e1d"
export MINER_PUBLIC_KEY="f90ef36c9a3da8fbb0cb8d5bf421c15862eeed62b042818762492f2488963e1d"

# Node configuration
export NODE_HOST="localhost:1848"

# Benchmark parameters
export BENCHMARK_DURATION=60  # seconds
export CPU_THREADS="1 2 4 8"
export STRATUM_CONNECTIONS="10 50 100"

# Load paths
source "$SCRIPT_DIR/paths.env"
EOF

print_status "Created benchmark configuration"

# Summary
echo -e "\n${GREEN}==================================="
echo "Build Setup Complete!"
echo "===================================${NC}"
echo
echo "Executable paths saved to: $SCRIPT_DIR/paths.env"
echo "Benchmark config saved to: $SCRIPT_DIR/benchmark-config.env"
echo
echo "To run benchmarks:"
echo "  cd $SCRIPT_DIR"
echo "  source benchmark-config.env"
echo "  ./quick-test.sh"
echo
echo "Haskell exe: \$HASKELL_MINING_CLIENT"
echo "Rust exe: \$RUST_MINING_CLIENT"