# AGENTS.md -- Matrix MCP Server R2 (Rust)

Unified Rust Matrix gateway for the Agent-Matrix fleet. Exposes 20 operational Matrix MCP tools via Streamable HTTP transport (`/mcp`) that transparently support Cross-Signed E2EE by default (with automatic pre-flight `/keys/query` fallback to unencrypted rooms when an invitee lacks E2EE keys), plus 2 E2EE management tools (`bootstrap-e2ee` and `get-encryption-status`), and serves as the single Matrix client and SQLite crypto store owner per agent container (with Stage 2 `003-integrated-matrix-bot` absorbing `matrix-bot` into `src/bot/`).

## Stack

- **Language**: Rust (edition 2021; compiling pin 1.88 until T001 bumps to >= 1.96)
- **Async**: Tokio
- **HTTP**: Axum 0.8, Tower, tower-http (CORS, tracing)
- **MCP**: rmcp 1.2 (StreamableHttpService + LocalSessionManager at `/mcp`)
- **Matrix**: matrix-sdk (compiling pin 0.10 until T001 bumps to 0.19 with `rustls-tls`, `e2e-encryption`, `bundled-sqlite`), ruma
- **Schemas**: schemars, serde, serde_json
- **Errors**: thiserror, anyhow

## Key Commands

```bash
# Build
cargo build                              # Debug
cargo build --release                    # Release (LTO + strip enabled)
cargo build --features e2ee              # With E2EE (planned)

# Run
cargo run                                # Default: HTTP on port 3000
cargo run -- --version                   # Show version

# Test
cargo test                               # All tests
cargo test --test integration_tests      # Integration only

# Lint and format
cargo clippy                             # Lints (warn level)
cargo fmt                                # Rustfmt defaults

# Docker
docker build -t matrix-mcp-server-r2 .
docker run --env-file .env -p 3000:3000 matrix-mcp-server-r2

# MCP Inspector (protocol testing without Matrix)
SKIP_MATRIX_INIT=true cargo run
# Then: npx @modelcontextprotocol/inspector -> http://localhost:3000/mcp
```

## Project Structure

```
src/
  main.rs              -- Axum app: /health, /mcp, graceful shutdown
  lib.rs
  config.rs            -- Config from env vars (OAuth/HTTPS/E2EE fields present but partial)
  error.rs
  auth.rs              -- MatrixAuthContext (minimal; from_config helper)
  matrix/
    client.rs          -- create_matrix_client, background sync loop with backoff
    cache.rs           -- TTL client cache (15-min TTL, periodic cleanup)
  mcp/
    server.rs          -- MatrixMcpServer: #[tool_router], ~20 #[tool] methods
tests/
  config_tests.rs, cache_tests.rs, integration_tests.rs, server_tool_list_tests.rs
spec/
  matrix-mcp-v1-v2.yml              -- v1/v2 API spec
  E2EE-Prep-Checklist-for-Rust-MCP-Server.md
reference/
  ts-source/           -- Snapshot of TypeScript implementation for reference
  docs/                -- Plans, design, tools spec, ops manual
.a0proj/instructions/project-instructions.md  -- Agent Zero project constraints
.cursor/rules/rust-e2ee.mdc                   -- Cursor rule for E2EE feature gating
```

## Configuration

Copy `.env.example` to `.env`. Key variables:
- `MATRIX_HOMESERVER_URL` -- Matrix server URL
- `MATRIX_USERNAME` / `MATRIX_PASSWORD` -- Login credentials
- `RUST_LOG` -- Log level (e.g., `info`, `debug`)
- `SKIP_MATRIX_INIT` -- Set `true` for protocol-only testing without Matrix

OAuth/HTTPS/E2EE env vars exist in config.rs but are partially implemented.

## Architecture Decisions

- **Streamable HTTP MCP**: Uses rmcp's StreamableHttpService (not raw SSE), mounted at `/mcp` on Axum
- **Shared Matrix client**: Single client with background sync, NOT per-tool ephemeral clients
- **TTL client cache**: 15-min TTL with cleanup task; primed for multi-identity scenarios
- **SKIP_MATRIX_INIT**: Stub client for MCP Inspector / protocol testing without live Matrix
- **Background sync**: Loops sync_once with backoff so room state stays fresh
- **Tool macros**: `#[tool_router]` + `#[tool]` on impl blocks; camelCase JSON via serde rename
- **No OAuth yet**: Config fields exist but auth.rs is minimal -- OAuth is planned, not implemented
- **Release profile**: LTO + strip + codegen-units=1 for small/fast binary

## Boundaries

### Always Do
- Run `cargo clippy` and `cargo fmt` before committing
- Run `cargo test` to verify nothing breaks
- Use camelCase for tool input JSON fields (serde rename, matches TS/agent expectations)
- Keep `reference/ts-source/` as-is for comparison -- don't modify it

### Ask First
- Before adding new Cargo dependencies
- Before modifying the MCP transport layer
- Before changing tool API signatures (callers depend on them)
- Before implementing E2EE (follow `spec/E2EE-Prep-Checklist-for-Rust-MCP-Server.md`)

### Never Do
- Commit `.env` files with real credentials
- Use `unwrap()` in tool implementations -- use proper error handling (thiserror/anyhow)
- Break camelCase tool input naming convention
- Modify `reference/` directory (it's a frozen snapshot)
