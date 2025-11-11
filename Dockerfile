# Multi-stage Dockerfile for Mantra SDK
# Optimized for production deployment with security and size considerations

####################
# BUILD STAGE
####################
FROM rust:1.75-slim-bullseye AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create app user for security
RUN groupadd -r mantra && useradd -r -g mantra mantra

# Set working directory
WORKDIR /usr/src/app

# Copy dependency manifests
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to cache dependencies
RUN mkdir src && \
    echo "fn main() {println!(\"Dummy main for dependency caching\");}" > src/main.rs && \
    echo "" > src/lib.rs

# Build dependencies (this layer will be cached unless Cargo.toml changes)
RUN cargo build --release --features mcp,performance,security,resilience && \
    rm -rf src/

# Copy source code
COPY src/ ./src/
COPY config/ ./config/
COPY docs/ ./docs/
COPY examples/ ./examples/

# Build the actual application
RUN cargo build --release --features mcp,performance,security,resilience

# Build TUI variant
RUN cargo build --release --bin mantra-dex-tui --features tui-dex

####################
# RUNTIME STAGE
####################
FROM debian:bullseye-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    curl \
    jq \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create app user
RUN groupadd -r mantra && useradd -r -g mantra -d /app -s /sbin/nologin mantra

# Create necessary directories
RUN mkdir -p /app/config /app/data /app/logs \
    && chown -R mantra:mantra /app

# Copy binaries from builder
COPY --from=builder /usr/src/app/target/release/mcp-server /usr/local/bin/mcp-server
COPY --from=builder /usr/src/app/target/release/mantra-dex-tui /usr/local/bin/mantra-dex-tui

# Copy configuration files
COPY --from=builder /usr/src/app/config/ /app/config/
COPY --from=builder /usr/src/app/docs/ /app/docs/

# Set proper permissions
RUN chmod +x /usr/local/bin/mcp-server /usr/local/bin/mantra-dex-tui \
    && chown -R mantra:mantra /app

# Switch to non-root user
USER mantra
WORKDIR /app

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Expose ports
EXPOSE 8080 8443

# Default command
CMD ["mcp-server", "--transport", "http", "--port", "8080"]

####################
# MCP SERVER VARIANT
####################
FROM runtime AS mcp-server
ENTRYPOINT ["mcp-server"]
CMD ["--transport", "http", "--port", "8080"]

####################
# TUI VARIANT
####################
FROM runtime AS tui
ENTRYPOINT ["mantra-dex-tui"]
CMD ["--help"]

####################
# DEVELOPMENT STAGE
####################
FROM builder AS development

# Install development tools
RUN cargo install cargo-watch cargo-audit cargo-outdated

# Install additional debugging tools
RUN apt-get update && apt-get install -y \
    gdb \
    strace \
    valgrind \
    && rm -rf /var/lib/apt/lists/*

# Set development environment
ENV RUST_LOG=debug
ENV RUST_BACKTRACE=1

WORKDIR /usr/src/app

# Default development command
CMD ["cargo", "run", "--bin", "mcp-server", "--features", "mcp,performance,security,resilience"]

####################
# TESTING STAGE
####################
FROM builder AS testing

# Install test dependencies
RUN cargo install cargo-tarpaulin cargo-llvm-cov

# Run tests
RUN cargo test --features mcp,performance,security,resilience
RUN cargo test --features tui-dex

# Generate coverage report
RUN cargo tarpaulin --out xml --features mcp,performance,security,resilience

# Copy test results
COPY --from=testing /usr/src/app/cobertura.xml /app/coverage.xml