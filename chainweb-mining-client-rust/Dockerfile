# Multi-stage build for minimal image size
# Build stage with Rust toolchain
FROM --platform=$BUILDPLATFORM rust:alpine AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    openssl-libs-static \
    pkgconfig \
    ca-certificates

# Set up cross-compilation
ARG TARGETPLATFORM
RUN case "$TARGETPLATFORM" in \
    "linux/amd64") echo "x86_64-unknown-linux-musl" > /target.txt ;; \
    "linux/arm64") echo "aarch64-unknown-linux-musl" > /target.txt ;; \
    *) echo "Unsupported platform: $TARGETPLATFORM" && exit 1 ;; \
    esac

# Install nightly toolchain and the target
RUN rustup toolchain install nightly && \
    rustup default nightly && \
    rustup target add --toolchain nightly $(cat /target.txt)

# Create app directory
WORKDIR /app

# Copy manifests first for better caching
COPY Cargo.toml Cargo.lock ./

# Create dummy source files to cache dependencies
RUN mkdir -p src benches && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub fn lib() {}" > src/lib.rs && \
    echo "fn main() {}" > benches/mining_performance.rs && \
    cargo build --release --target $(cat /target.txt) && \
    rm -rf src benches

# Copy actual source code
COPY src ./src

# Build with maximum optimizations
# Build with maximum optimizations
# Remove target-cpu=native for cross-compilation compatibility
ENV RUSTFLAGS="-C opt-level=3 -C lto=fat -C codegen-units=1 -C strip=symbols -C panic=abort"
RUN cargo build --release --target $(cat /target.txt) && \
    cp target/$(cat /target.txt)/release/chainweb-mining-client /chainweb-mining-client

# Runtime stage - using scratch for minimal size
FROM scratch

# Copy only the binary and SSL certificates
COPY --from=builder /chainweb-mining-client /chainweb-mining-client
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Set entrypoint
ENTRYPOINT ["/chainweb-mining-client"]