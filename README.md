# Matrix-MCP-SVR-R2

> Rust implementation of the Matrix MCP Server -- a drop-in replacement for
> [matrix-mcp-server](https://github.com/user/matrix-mcp-server) (TypeScript)

## Status: Phase 2 -- v1 Implementation (API parity)

| Phase | Description | Status |
|-------|-------------|--------|
| 0 | Project scaffolding | Done |
| 1 | Spec writing | Done |
| 2 | v1 Implementation -- API parity | Done |
| 3 | v2 Implementation -- E2EE | Not started |
| 4 | Integration testing & deployment | Not started |

## What This Is

A Rust rewrite of the Matrix MCP (Model Context Protocol) server that enables
AI agents to interact with Matrix rooms, messages, and users. The server
exposes Matrix operations as MCP tools over the Streamable HTTP transport.

## Why Rust?

- **Performance**: Single binary, sub-millisecond tool dispatch
- **Safety**: Memory safety without GC; ideal for long-running server processes
- **E2EE**: Native access to `matrix-sdk-crypto` (Olm/Megolm) -- no FFI needed
- **Deployment**: Static binary, no Node.js runtime, no `node_modules/`

## Architecture

```
  MCP Client        POST /mcp        Axum HTTP         rmcp
  (Agent Zero) ----------------->  (CORS, trace) --> StreamableHttpService
                                                         |
                                                    ServerHandler
                                                    (20 MCP tools)
                                                         |
                                                    matrix-sdk Client
                                                         |
                                                    Matrix Homeserver
```

## Tool Parity (v1): 20 tools

| Category | Tools |
|----------|-------|
| Rooms | list-joined-rooms, get-room-info, create-room, join-room, leave-room |
| Messages | get-room-messages, get-messages-by-date, send-message, send-direct-message |
| Members | get-room-members, invite-user, identify-active-users |
| Profiles | get-user-profile, get-my-profile, get-all-users |
| Admin | set-room-name, set-room-topic |
| Discovery | search-public-rooms, get-notification-counts, get-direct-messages |

## Tech Stack

| Component | Choice |
|-----------|--------|
| Language | Rust 2021 edition |
| MCP SDK | rmcp 1.2 (official Rust MCP SDK) |
| HTTP | axum + tower-http |
| Matrix SDK | matrix-sdk 0.10 |
| Serialization | serde + schemars |
| Error Handling | thiserror |
| Logging | tracing |
| E2EE (v2) | matrix-sdk-crypto (behind `e2ee` feature) |

## Quick Start

```bash
# Clone and configure
cp .env.example .env
# Edit .env with your Matrix homeserver URL, user ID, and access token

# Build and run
cargo build
cargo run

# The MCP endpoint is at http://localhost:3000/mcp
# Health check at http://localhost:3000/health
```

## Publishing (GHCR)

Official images are built by GitHub Actions and pushed to **GitHub Container Registry** when you push a **semver git tag** whose version matches [`Cargo.toml`](Cargo.toml) `version` (the same value baked into the binary as `CARGO_PKG_VERSION`).

1. Commit your changes and merge to the default branch.
2. Create an annotated tag with a `v` prefix, e.g. for `0.1.0`:
   - `git tag -a v0.1.0 -m "matrix-mcp-server-r2 0.1.0"`
   - `git push origin v0.1.0`
3. Open the repo on GitHub → **Actions** and confirm the **Publish Docker image** workflow succeeds.
4. Under **Packages** (or the workflow summary), pull the image (use your GitHub username or org, lowercased):

```bash
docker pull ghcr.io/<github-owner>/matrix-mcp-server-r2:0.1.0
```

The `:latest` tag is updated on each tag push that matches `v*`. Image tags like `0.1.0` match the server version reported by `GET /health` and `matrix-mcp-server-r2 --version`.

**Manual workflow run:** You can use **Actions → Publish Docker image → Run workflow** and select a **tag** as the ref (not only `main`) if you need to rebuild an existing release.

**Optional:** Create a **GitHub Release** from the same tag for release notes; it is not required for GHCR.

## Configuration

All configuration is via environment variables (or a `.env` file).
See `.env.example` for the full list. Required variables:

| Variable | Description |
|----------|-------------|
| `MATRIX_HOMESERVER_URL` | Matrix homeserver URL (e.g., `https://matrix.example.com`) |
| `MATRIX_USER_ID` | Matrix user ID (e.g., `@bot:example.com`) |
| `MATRIX_ACCESS_TOKEN` | Matrix access token |
| `PORT` | Server port (default: 3000) |

## Testing

```bash
# Run all tests (unit + integration with wiremock mock homeserver)
cargo test

# Run only integration tests
cargo test --test integration_tests
```

### MCP Inspector Validation

The MCP Inspector lets you interactively explore the server's protocol
compliance -- `initialize`, `tools/list`, session management, and tool calls.

**Option A -- Protocol-only (no Matrix homeserver needed):**

```bash
# Terminal 1: start with SKIP_MATRIX_INIT (no real Matrix connection)
SKIP_MATRIX_INIT=true cargo run

# Terminal 2: launch the inspector
npx @modelcontextprotocol/inspector
# connect to http://localhost:3000/mcp
```

In this mode the server starts instantly, `initialize` and `tools/list` work
fully, but tool calls return Matrix connection errors. Good for verifying the
MCP framing.

**Option B -- Full stack (requires a Matrix homeserver):**

```bash
# Terminal 1: start with real credentials
MATRIX_HOMESERVER_URL=https://matrix.example.com \
MATRIX_USER_ID=@bot:example.com \
MATRIX_ACCESS_TOKEN=syt_... \
cargo run

# Terminal 2:
npx @modelcontextprotocol/inspector
# connect to http://localhost:3000/mcp
```

In this mode tool calls execute against the real homeserver.

## Project Structure

```
src/
  main.rs              # Entry point, axum/rmcp wiring, graceful shutdown
  lib.rs               # Library re-exports
  config.rs            # Configuration from environment variables
  error.rs             # Error types
  auth.rs              # Per-request auth context (future)
  matrix/
    client.rs          # matrix-sdk client creation and background sync
    cache.rs           # TTL-based client cache
  mcp/
    server.rs          # MatrixMcpServer: all 20 tools + ServerHandler
reference/
  ts-source/           # Original TypeScript implementation (reference)
  docs/                # Plans, specs, operational docs
spec/
  matrix-mcp-v1-v2.yml # v1/v2 API specification
tests/
  config_tests.rs      # Configuration validation tests
  cache_tests.rs       # Client cache tests
  server_tool_list_tests.rs  # Tool registration tests
```

## Parent Project

This server is a component of the **Agent-Matrix** sovereign agent fleet.

---

*Started: 2026-03-11 | Human-AI Collaboration Project*
