# Stage 1: Build the executable using cabal
FROM haskell:9.6-slim AS builder

# Install required build dependencies and clean up in same layer
RUN apt-get update && apt-get install -y --no-install-recommends libssl-dev pkg-config && rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*

WORKDIR /app

# Copy dependency information
COPY chainweb-mining-client.cabal ./
COPY cabal.project ./
COPY Setup.hs ./

# Download and install dependencies. This layer is cached.
RUN cabal update && cabal build --dependencies-only --ghc-options="-O2 -split-sections -optl-Wl,--gc-sections"

# Copy the rest of the source code
COPY src/ ./src
COPY main/ ./main
COPY test/ ./test
COPY README.md ./
COPY CHANGELOG.md ./
COPY LICENSE ./

# Build and install the executable to a predictable location
RUN cabal install exe:chainweb-mining-client --install-method=copy --overwrite-policy=always --installdir=/app/bin --ghc-options="-O2 -split-sections -optl-Wl,--gc-sections" --enable-executable-stripping

# Strip the binary further to reduce size
RUN strip /app/bin/chainweb-mining-client

# Stage 2: Create the final image using Alpine
FROM debian:bookworm-slim

# Install only runtime dependencies and clean up in same layer
RUN apt-get update && \
  apt-get install -y --no-install-recommends libssl3 && \
  apt-get autoremove -y && \
  apt-get clean && \
  rm -rf /var/lib/apt/lists/* \
    /tmp/* \
    /var/tmp/* \
    /usr/share/doc/* \
    /usr/share/man/* \
    /usr/share/locale/* \
    /var/cache/debconf/* \
    /usr/share/info/* \
    /usr/share/lintian/* \
    /usr/share/common-licenses/*

WORKDIR /app

# Create a non-root user
RUN addgroup --system --gid 10001 appgroup && adduser --system --uid 10001 --ingroup appgroup appuser

# Copy the executable from the builder stage
COPY --from=builder /app/bin/chainweb-mining-client /app/chainweb-mining-client

# Change ownership and make executable
RUN chown appuser:appgroup /app/chainweb-mining-client
RUN chmod +x /app/chainweb-mining-client

# Set the user to non-root for security
USER appuser:appgroup

ENTRYPOINT ["/app/chainweb-mining-client"]
