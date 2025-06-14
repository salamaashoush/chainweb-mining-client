# Justfile Commands Guide

This document provides a comprehensive guide to all available `just` commands for the Chainweb Mining Client Rust implementation.

## Prerequisites

Install `just` command runner:
```bash
# macOS
brew install just

# Linux (Ubuntu/Debian)
wget -qO - 'https://proget.makedeb.org/debian-feeds/prebuilt-mpr/pubkey.gpg' | gpg --dearmor | sudo tee /usr/share/keyrings/prebuilt-mpr-archive-keyring.gpg 1> /dev/null
echo "deb [arch=all,amd64,arm64 signed-by=/usr/share/keyrings/prebuilt-mpr-archive-keyring.gpg] https://proget.makedeb.org prebuilt-mpr $(lsb_release -cs)" | sudo tee /etc/apt/sources.list.d/prebuilt-mpr.list
sudo apt update && sudo apt install just

# Or via cargo
cargo install just
```

## Quick Reference

```bash
just                    # Run default checks
just help              # Show all available commands
just dev-full          # Complete development workflow
just prod-ready         # Production readiness check
```

## Core Development Commands

### Code Quality & Building

| Command | Description |
|---------|-------------|
| `just check` | Run all code quality checks (default) |
| `just ci` | Run all CI checks including tests and release build |
| `just dev-check` | Quick development checks (format + lint) |
| `just build` | Build the project |
| `just build-release` | Build release version |
| `just test` | Run tests |

### Code Formatting & Linting

| Command | Description |
|---------|-------------|
| `just fmt` | Format code with rustfmt |
| `just fmt-check` | Check if code is formatted |
| `just lint` | Run clippy linter |
| `just fix` | Auto-fix formatting and clippy issues |
| `just typos` | Check for typos |
| `just unused-deps` | Check for unused dependencies |

### Development Tools

| Command | Description |
|---------|-------------|
| `just dev-setup` | Install system dependencies and development tools |
| `just dev-verify` | Verify all development tools are installed |
| `just watch` | Watch for changes and run checks |
| `just docs` | Generate and open documentation |
| `just audit` | Run security audit |

## Testing Commands

### Unit & Integration Testing

| Command | Description |
|---------|-------------|
| `just test` | Run all tests |
| `just test-one <TEST>` | Run specific test |
| `just test-stratum-unit` | Run Stratum unit tests only |

### Compatibility Testing

| Command | Description |
|---------|-------------|
| `just test-compat-all` | Run all compatibility tests vs Haskell |
| `just test-compat-stratum` | Test Stratum compatibility |
| `just test-compat-workers` | Test all worker types |
| `just test-compat-external` | Test external worker |
| `just test-compat-on-demand` | Test on-demand worker |

### Stratum Protocol Testing

| Command | Description |
|---------|-------------|
| `just test-stratum [PORT]` | Full Stratum protocol compatibility test |
| `just test-stratum-no-node` | Test Stratum without requiring a node |
| `just test-stratum-unit` | Stratum unit tests only |

## End-to-End Stress Testing

### Comprehensive Testing

| Command | Description | Default | Example |
|---------|-------------|---------|---------|
| `just e2e-stress [DURATION] [WORKERS]` | Run comprehensive E2E stress tests | 60s, 4 workers | `just e2e-stress 120 8` |
| `just e2e-stress-quick` | Run quick E2E stress tests | 30s, 2 workers | |
| `just e2e-test-all` | Run all E2E tests (programmatic) | | |
| `just e2e-test <TEST>` | Run specific E2E test | | `just e2e-test cpu_stress` |

### Individual Component Testing

| Command | Description | Default | 
|---------|-------------|---------|
| `just e2e-stress-cpu [DURATION] [WORKERS]` | CPU mining stress test only | 60s, 4 workers |
| `just e2e-stress-stratum [DURATION] [WORKERS]` | Stratum server stress test only | 45s, 8 workers |
| `just e2e-stress-external [DURATION] [WORKERS]` | External worker stress test only | 30s, 6 workers |

### Environment Management

| Command | Description |
|---------|-------------|
| `just e2e-setup` | Setup E2E testing environment |
| `just e2e-cleanup` | Cleanup E2E testing environment |

## Performance Benchmarking

### Benchmark Suites

| Command | Description |
|---------|-------------|
| `just bench` | Run standard benchmarks |
| `just bench-comprehensive` | Run comprehensive performance benchmarks |
| `just bench-stress` | Run stress testing benchmarks |
| `just bench-property` | Run property-based testing benchmarks |
| `just bench-all-suites` | Run all benchmark suites |

### Advanced Benchmarking

| Command | Description | Example |
|---------|-------------|---------|
| `just bench-quick` | Run quick benchmarks (fewer samples) | |
| `just bench-save <baseline>` | Save benchmark baseline | `just bench-save main` |
| `just bench-compare <baseline>` | Compare against baseline | `just bench-compare main` |
| `just bench-report [output]` | Generate HTML reports | `just bench-report ./reports` |

## Monitoring & Production

### Monitoring

| Command | Description |
|---------|-------------|
| `just monitoring-status` | Show current monitoring status |
| `just dev-with-monitoring` | Start development with monitoring enabled |

### Production Workflows

| Command | Description |
|---------|-------------|
| `just prod-ready` | Complete production readiness check |
| `just dev-full` | Full development workflow |
| `just release <TAG>` | Complete release workflow |

## Docker Commands

### Building

| Command | Description | Example |
|---------|-------------|---------|
| `just docker-build [TAG] [TYPE]` | Build Docker image | `just docker-build v1.0 scratch` |
| `just docker-build-all [TAG]` | Build all Docker variants | `just docker-build-all v1.0` |
| `just docker-test [TAG]` | Test Docker image | `just docker-test latest` |
| `just docker-clean` | Clean Docker build cache | |

### Types
- `scratch`: Minimal image based on scratch
- `distroless`: Google distroless base image

## Utility Commands

| Command | Description |
|---------|-------------|
| `just clean` | Clean build artifacts |
| `just install` | Install the binary |
| `just check-file <FILE>` | Check specific file |

## Workflow Examples

### Daily Development
```bash
# Quick development cycle
just dev-check          # Fast checks
just test               # Run tests
just build              # Build project

# Or all at once
just dev-full           # Complete workflow
```

### Before Committing
```bash
just ci                 # All CI checks
just e2e-stress-quick   # Quick E2E validation
```

### Performance Testing
```bash
just bench-comprehensive  # Performance benchmarks
just e2e-stress 300 8    # Extended stress test
just monitoring-status   # Check monitoring
```

### Production Deployment
```bash
just prod-ready         # Full production check
just release v1.2.0     # Create release
```

### Debugging & Development
```bash
just dev-with-monitoring     # Development with monitoring
just test-one <failing_test> # Debug specific test
just watch                   # Continuous development
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `TEST_DURATION` | E2E test duration (seconds) | 60 |
| `WORKER_COUNT` | Number of concurrent workers | 4 |
| `IMAGE_NAME` | Docker image name | `salamaashoush/chainweb-mining-client` |
| `RUST_LOG` | Rust logging level | `info` |

### Examples
```bash
# Custom stress test
TEST_DURATION=180 WORKER_COUNT=12 just e2e-stress

# Debug logging
RUST_LOG=debug just dev-with-monitoring

# Custom Docker image
IMAGE_NAME=myregistry/mining-client just docker-build
```

## Troubleshooting

### Common Issues

#### Tool Missing
```bash
just dev-setup          # Install all tools
just dev-verify         # Verify installation
```

#### Docker Issues
```bash
just docker-clean       # Clean Docker cache
just e2e-cleanup        # Clean test environment
```

#### Test Failures
```bash
just clean              # Clean build artifacts
just check              # Run quality checks
just test-one <test>    # Debug specific test
```

#### Performance Issues
```bash
just bench-save baseline    # Save current performance
# Make changes
just bench-compare baseline # Compare performance
```

## Integration with IDEs

### VS Code
Add to `.vscode/tasks.json`:
```json
{
    "version": "2.0.0",
    "tasks": [
        {
            "label": "just check",
            "type": "shell",
            "command": "just",
            "args": ["check"],
            "group": "build"
        }
    ]
}
```

### IntelliJ/CLion
- Add external tool: `just` with argument `check`
- Set working directory to project root
- Add keyboard shortcut for quick access

## Contributing

When adding new justfile commands:

1. **Add to help section**: Update the help message
2. **Follow naming conventions**: Use kebab-case
3. **Add documentation**: Document parameters and examples
4. **Test thoroughly**: Ensure commands work in different environments
5. **Update this guide**: Keep documentation current

## Performance Tips

- Use `just dev-check` for quick iterations
- Use `just e2e-stress-quick` for fast E2E validation  
- Use `just bench-quick` for rapid performance checks
- Use `just watch` for continuous development
- Cache Docker layers by using specific tags

This justfile provides a comprehensive development experience for the Chainweb Mining Client, from code quality to production deployment.