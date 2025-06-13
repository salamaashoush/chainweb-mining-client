#!/bin/bash
# Cross-platform dependency installation script for chainweb-mining-client-rust

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
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

# Detect the Linux distribution
detect_distro() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        DISTRO=$ID
        VERSION=$VERSION_ID
    elif command -v lsb_release >/dev/null 2>&1; then
        DISTRO=$(lsb_release -si | tr '[:upper:]' '[:lower:]')
        VERSION=$(lsb_release -sr)
    elif [ -f /etc/redhat-release ]; then
        DISTRO="rhel"
        VERSION=$(cat /etc/redhat-release | grep -o '[0-9]\+\.[0-9]\+' | head -1)
    else
        print_error "Cannot detect Linux distribution"
        exit 1
    fi
    
    print_step "Detected distribution: $DISTRO $VERSION"
}

# Check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check if running as root
check_sudo() {
    if [ "$EUID" -eq 0 ]; then
        print_warning "Running as root. This is not recommended for development."
        SUDO_CMD=""
    else
        SUDO_CMD="sudo"
    fi
}

# Install Rust development tools via cargo
install_rust_tools() {
    print_step "Installing Rust development tools..."
    
    local rust_tools=(
        "cargo-machete"      # Unused dependency detection
        "typos-cli"          # Typo checking
        "cargo-audit"        # Security auditing
        "cargo-llvm-cov"     # Code coverage
    )
    
    for tool in "${rust_tools[@]}"; do
        if ! command_exists "$tool"; then
            print_step "Installing $tool..."
            cargo install "$tool" || print_warning "Failed to install $tool"
        else
            print_success "$tool already installed"
        fi
    done
}

# Install system dependencies for Arch Linux
install_arch_deps() {
    print_step "Installing dependencies for Arch Linux..."
    
    local arch_packages=(
        "expect"           # Expect script interpreter
        "inetutils"        # Telnet client
        "gnu-netcat"       # Network connectivity testing
        "docker"           # Docker engine
        "docker-buildx"    # Docker buildx plugin
        "just"             # Command runner
        "git"              # Version control
        "base-devel"       # Build tools
    )
    
    # Update package database
    $SUDO_CMD pacman -Sy
    
    # Install packages that aren't already installed
    local to_install=()
    for pkg in "${arch_packages[@]}"; do
        if ! pacman -Qi "$pkg" >/dev/null 2>&1; then
            to_install+=("$pkg")
        fi
    done
    
    if [ ${#to_install[@]} -gt 0 ]; then
        print_step "Installing packages: ${to_install[*]}"
        $SUDO_CMD pacman -S --noconfirm "${to_install[@]}"
    else
        print_success "All Arch packages already installed"
    fi
}

# Install system dependencies for Ubuntu/Debian
install_ubuntu_deps() {
    print_step "Installing dependencies for Ubuntu/Debian..."
    
    local ubuntu_packages=(
        "expect"           # Expect script interpreter
        "telnet"           # Telnet client
        "netcat-openbsd"   # Network connectivity testing
        "docker.io"        # Docker engine
        "git"              # Version control
        "build-essential"  # Build tools
        "pkg-config"       # Package configuration
        "libssl-dev"       # SSL development libraries
        "curl"             # HTTP client
        "wget"             # Download tool
    )
    
    # Update package database
    $SUDO_CMD apt-get update
    
    # Install packages
    print_step "Installing packages: ${ubuntu_packages[*]}"
    $SUDO_CMD apt-get install -y "${ubuntu_packages[@]}"
    
    # Install just if not available in repos
    if ! command_exists just; then
        print_step "Installing just via cargo..."
        cargo install just
    fi
}

# Install system dependencies for Fedora/RHEL/CentOS
install_fedora_deps() {
    print_step "Installing dependencies for Fedora/RHEL..."
    
    local fedora_packages=(
        "expect"           # Expect script interpreter
        "telnet"           # Telnet client
        "nc"               # Network connectivity testing
        "docker"           # Docker engine
        "git"              # Version control
        "gcc"              # C compiler
        "gcc-c++"          # C++ compiler
        "make"             # Build tool
        "pkgconf-pkg-config" # Package configuration
        "openssl-devel"    # SSL development libraries
        "curl"             # HTTP client
        "wget"             # Download tool
    )
    
    # Determine package manager
    if command_exists dnf; then
        PKG_MGR="dnf"
    elif command_exists yum; then
        PKG_MGR="yum"
    else
        print_error "No supported package manager found (dnf/yum)"
        return 1
    fi
    
    # Install packages
    print_step "Installing packages with $PKG_MGR: ${fedora_packages[*]}"
    $SUDO_CMD "$PKG_MGR" install -y "${fedora_packages[@]}"
    
    # Install just if not available
    if ! command_exists just; then
        print_step "Installing just via cargo..."
        cargo install just
    fi
}

# Install system dependencies for openSUSE
install_opensuse_deps() {
    print_step "Installing dependencies for openSUSE..."
    
    local opensuse_packages=(
        "expect"              # Expect script interpreter
        "telnet"              # Telnet client
        "netcat-openbsd"      # Network connectivity testing
        "docker"              # Docker engine
        "git"                 # Version control
        "gcc"                 # C compiler
        "gcc-c++"             # C++ compiler
        "make"                # Build tool
        "pkg-config"          # Package configuration
        "libopenssl-devel"    # SSL development libraries
        "curl"                # HTTP client
        "wget"                # Download tool
    )
    
    # Install packages
    print_step "Installing packages: ${opensuse_packages[*]}"
    $SUDO_CMD zypper install -y "${opensuse_packages[@]}"
    
    # Install just if not available
    if ! command_exists just; then
        print_step "Installing just via cargo..."
        cargo install just
    fi
}

# Configure Docker
setup_docker() {
    if command_exists docker; then
        print_step "Configuring Docker..."
        
        # Enable and start Docker service
        if command_exists systemctl; then
            $SUDO_CMD systemctl enable docker
            $SUDO_CMD systemctl start docker
        fi
        
        # Add user to docker group
        if [ "$SUDO_CMD" = "sudo" ]; then
            $SUDO_CMD usermod -aG docker "$USER"
            print_warning "Added $USER to docker group. Please log out and back in for changes to take effect."
        fi
        
        print_success "Docker configured"
    fi
}

# Install Rust if not present
install_rust() {
    if ! command_exists rustc; then
        print_step "Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
        print_success "Rust installed"
    else
        print_success "Rust already installed"
    fi
}

# Verify installations
verify_installation() {
    print_step "Verifying installations..."
    
    local required_tools=(
        "rustc"
        "cargo"
        "expect"
        "telnet"
        "nc"
        "git"
        "docker"
    )
    
    local optional_tools=(
        "just"
        "cargo-machete"
        "typos"
        "cargo-audit"
        "cargo-llvm-cov"
    )
    
    local missing_required=()
    local missing_optional=()
    
    # Check required tools
    for tool in "${required_tools[@]}"; do
        if command_exists "$tool"; then
            print_success "$tool ✓"
        else
            missing_required+=("$tool")
        fi
    done
    
    # Check optional tools
    for tool in "${optional_tools[@]}"; do
        if command_exists "$tool"; then
            print_success "$tool ✓"
        else
            missing_optional+=("$tool")
        fi
    done
    
    # Report results
    if [ ${#missing_required[@]} -gt 0 ]; then
        print_error "Missing required tools: ${missing_required[*]}"
        return 1
    fi
    
    if [ ${#missing_optional[@]} -gt 0 ]; then
        print_warning "Missing optional tools: ${missing_optional[*]}"
        print_warning "Run 'cargo install ${missing_optional[*]}' to install them"
    fi
    
    print_success "All required dependencies are installed!"
}

# Main installation function
main() {
    echo "🛠️  Chainweb Mining Client - Dependency Installation Script"
    echo "========================================================="
    
    # Initial checks
    detect_distro
    check_sudo
    
    # Install Rust first if needed
    install_rust
    
    # Install system dependencies based on distribution
    case "$DISTRO" in
        "arch"|"manjaro")
            install_arch_deps
            ;;
        "ubuntu"|"debian"|"linuxmint"|"pop"|"elementary")
            install_ubuntu_deps
            ;;
        "fedora"|"rhel"|"centos"|"rocky"|"almalinux")
            install_fedora_deps
            ;;
        "opensuse"|"opensuse-leap"|"opensuse-tumbleweed"|"sles")
            install_opensuse_deps
            ;;
        *)
            print_warning "Unsupported distribution: $DISTRO"
            print_warning "Please install the following manually:"
            echo "  - expect"
            echo "  - telnet"
            echo "  - netcat"
            echo "  - docker"
            echo "  - git"
            echo "  - build tools (gcc, make, etc.)"
            ;;
    esac
    
    # Install Rust development tools
    install_rust_tools
    
    # Configure Docker
    setup_docker
    
    # Verify everything is installed
    verify_installation
    
    echo ""
    print_success "Setup complete! 🎉"
    echo ""
    echo "Next steps:"
    echo "  1. If Docker group was added, log out and back in"
    echo "  2. Run 'just check' to verify everything works"
    echo "  3. Run 'just test-stratum-unit' to test Stratum protocol"
    echo "  4. Run 'just test-stratum' for full integration testing"
}

# Handle script arguments
case "${1:-}" in
    "verify"|"check")
        verify_installation
        exit $?
        ;;
    "rust-only")
        install_rust_tools
        exit 0
        ;;
    "help"|"-h"|"--help")
        echo "Usage: $0 [command]"
        echo ""
        echo "Commands:"
        echo "  (none)      - Install all dependencies"
        echo "  verify      - Verify installations"
        echo "  rust-only   - Install only Rust tools"
        echo "  help        - Show this help"
        exit 0
        ;;
    "")
        main
        ;;
    *)
        print_error "Unknown command: $1"
        echo "Run '$0 help' for usage information"
        exit 1
        ;;
esac