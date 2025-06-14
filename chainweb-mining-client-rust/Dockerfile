# Multi-stage build for minimal image size
# Build stage with Rust toolchain
FROM rust:alpine AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    openssl-libs-static \
    pkgconfig \
    ca-certificates

# Create app directory
WORKDIR /app

# Copy manifests first for better caching
COPY Cargo.toml Cargo.lock ./

# Add musl target for the current platform
ARG TARGETPLATFORM
RUN case ${TARGETPLATFORM} in \
        "linux/amd64") rustup target add x86_64-unknown-linux-musl ;; \
        "linux/arm64") rustup target add aarch64-unknown-linux-musl ;; \
        *) echo "Unsupported platform: ${TARGETPLATFORM}"; exit 1 ;; \
    esac

# Set the build target based on platform
RUN case ${TARGETPLATFORM} in \
        "linux/amd64") echo "x86_64-unknown-linux-musl" > /target.txt ;; \
        "linux/arm64") echo "aarch64-unknown-linux-musl" > /target.txt ;; \
        *) echo "Unsupported platform: ${TARGETPLATFORM}"; exit 1 ;; \
    esac

# Create dummy source files to cache dependencies
RUN mkdir -p src benches && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub mod config; pub mod core; pub mod error; pub mod protocol; pub mod utils; pub mod workers;" > src/lib.rs && \
    mkdir -p src/config src/core src/error src/protocol src/utils src/workers && \
    echo "" > src/config/mod.rs && \
    echo "" > src/core/mod.rs && \
    echo "" > src/error/mod.rs && \
    echo "" > src/protocol/mod.rs && \
    echo "" > src/utils/mod.rs && \
    echo "" > src/workers/mod.rs && \
    echo "fn main() {}" > benches/mining_performance.rs && \
    echo "fn main() {}" > benches/protocol_performance.rs && \
    echo "fn main() {}" > benches/stratum_performance.rs && \
    echo "fn main() {}" > benches/config_performance.rs && \
    cargo build --release --target $(cat /target.txt) && \
    rm -rf src benches

# Copy actual source code
COPY src ./src
COPY benches ./benches

# Build with maximum optimizations (already configured in Cargo.toml)
RUN cargo build --release --target $(cat /target.txt) && \
    cp target/$(cat /target.txt)/release/chainweb-mining-client /chainweb-mining-client

# Runtime stage - using alpine for proper runtime support
FROM alpine:latest

# Install CA certificates for HTTPS connections
RUN apk add --no-cache ca-certificates

# Copy the binary
COPY --from=builder /chainweb-mining-client /chainweb-mining-client

# Make sure it's executable
RUN chmod +x /chainweb-mining-client

# Set entrypoint
ENTRYPOINT ["/chainweb-mining-client"]