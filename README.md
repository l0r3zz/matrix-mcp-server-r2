# Matrix-MCP-SVR-R2

> Rust implementation of the Matrix MCP Server -- a drop-in replacement for
> [matrix-mcp-server](https://github.com/user/matrix-mcp-server) (TypeScript)

## Status: Phase 2 -- v1 Implementation (API parity)

| Phase | Description | Status |
|-------|-------------|--------|
| 0 | Project scaffolding | Done |
| 1 | Spec writing | Done |
| 2 | v1 Implementation -- API parity | **In Progress** |
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
cargo test
```

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
