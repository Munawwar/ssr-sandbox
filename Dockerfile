# Dockerfile for local testing in a Linux environment
# Not needed for distribution - binaries are published to GitHub Releases
#
# Build: docker build -t ssr-sandbox .
# Test:  docker run --rm -v ./dist:/app/chunks:ro ssr-sandbox /app/chunks /app/chunks/entry.js '{}'

# Stage 1: Build
FROM rust:bookworm AS builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first for better caching
COPY Cargo.toml Cargo.lock* ./
COPY .cargo .cargo

# Create dummy src to cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub fn dummy() {}" > src/lib.rs

# Build dependencies (cached layer)
# Remove the dummy binary AND its fingerprint so cargo rebuilds with real source
RUN cargo build --release && \
    rm -rf src target/release/ssr-sandbox* target/release/.fingerprint/ssr-sandbox-*

# Copy actual source
COPY src src

# Build the real binary
RUN cargo build --release

# Stage 2: Runtime (distroless)
FROM gcr.io/distroless/cc-debian12:nonroot

# Copy the binary
COPY --from=builder /build/target/release/ssr-sandbox /usr/local/bin/ssr-sandbox

# Default working directory for chunks
WORKDIR /app

# Run as non-root
USER nonroot:nonroot

ENTRYPOINT ["/usr/local/bin/ssr-sandbox"]
