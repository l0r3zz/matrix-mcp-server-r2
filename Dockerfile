# =============================================================================
# Multi-stage Dockerfile for matrix-mcp-server-r2 (Rust)
# =============================================================================
# Produces a minimal image containing a single static binary.
#
# Usage:
#   docker build -t matrix-mcp-server-r2 .
#
# Extract the binary (for copying into an agent-zero container):
#   docker create --name mcp-build matrix-mcp-server-r2
#   docker cp mcp-build:/usr/local/bin/matrix-mcp-server-r2 ./matrix-mcp-server-r2
#   docker rm mcp-build
#
# Or run standalone:
#   docker run --env-file .env -p 3000:3000 matrix-mcp-server-r2
# =============================================================================

# ---- Stage 1: Build ----
# Toolchain floor is set by transitive deps (e.g. `darling`, `time` require rustc 1.88+;
# `base64ct` needs Cargo that understands edition2024).
FROM rust:1.88-bookworm AS builder

WORKDIR /build

# Makes the Rust/Cargo version visible in CI logs (must not be 1.82.x for edition2024 crates).
RUN rustc --version && cargo --version

# Cache dependency compilation: copy manifests first, build a dummy main,
# then copy real sources. This way cargo only re-downloads/re-compiles deps
# when Cargo.toml or Cargo.lock change.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() { println!("placeholder"); }' > src/main.rs && \
    echo 'pub mod config; pub mod error; pub mod auth; pub mod matrix; pub mod mcp;' > src/lib.rs && \
    mkdir -p src/matrix src/mcp && \
    touch src/config.rs src/error.rs src/auth.rs src/matrix/mod.rs src/matrix/client.rs \
          src/matrix/cache.rs src/mcp/mod.rs src/mcp/server.rs && \
    cargo build --release 2>/dev/null || true

# Now copy real source and rebuild
COPY src/ src/
RUN cargo build --release

# ---- Stage 2: Runtime ----
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/matrix-mcp-server-r2 /usr/local/bin/matrix-mcp-server-r2

EXPOSE 3000
ENV RUST_LOG=info

ENTRYPOINT ["matrix-mcp-server-r2"]
