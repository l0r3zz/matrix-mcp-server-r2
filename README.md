# Matrix-MCP-SVR-R2

> Rust implementation of the Matrix MCP Server -- a drop-in replacement for
> [matrix-mcp-server](https://github.com/mjknowles/matrix-mcp-server) (TypeScript)

## Latest release: 0.4.0

`v0.4.0` is the current release. The `Cargo.toml` version is `0.4.0`, and the git tag is `v0.4.0`.

### Image attachments

When `BOT_ENABLED=true`, a triggered prompt can carry one image file from a Matrix client. The server sends that image to the backend model that evaluates the prompt. Set `MODEL_ACCEPTS_IMAGES=true` only when that Agent Zero model accepts images. If the variable is unset or has any other value, the server posts one room error and does not evaluate the prompt. Other file types stay on the existing path. One image per message is supported.

### Invite retry

A trusted invite whose first join returns HTTP 403 because the user is not invited yet is retried while the invite remains, for at most 5 attempts. A ban, a missing room, an unsupported room version, and a 403 after the invite is gone stop after one attempt.

## Status

| Phase | Description | Status |
|-------|-------------|--------|
| 0 | Project scaffolding | Done |
| 1 | Spec writing | Done |
| 2 | v1 Implementation -- API parity | Done |
| 3 | v2 Implementation -- E2EE | Done (v0.2.0; gates T014 and T016 passed on agent0-5) |
| 4 | Integration testing & deployment | Done (v0.4.0) |

## Upgrading

| From | Guide |
|------|--------|
| v0.3.0 or v0.3.1 | [`docs/upgrade-to-v0.4.0.md`](docs/upgrade-to-v0.4.0.md) |
| v0.2.x with the separate Matrix bot still running | Agent-Matrix v0.93 integrated-bot upgrade, then [`docs/upgrade-to-v0.4.0.md`](docs/upgrade-to-v0.4.0.md) |
| v0.1.x on Agent-Matrix v0.91 | [`docs/upgrade-to-v0.2.0.md`](docs/upgrade-to-v0.2.0.md), then the v0.93 upgrade, then the v0.4.0 guide |

v0.4.0 keeps the existing Matrix device and crypto store. Restart only `run_mcp`. Leave `run_bot` stopped.

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
                                                    (21 + 2 MCP tools, 23 total)
                                                         |
                                                    matrix-sdk Client
                                                         |
                                                    Matrix Homeserver
```

## Tools: 21 + 2 (23 total)

The router registers 23 tools. Twenty-one operate on Matrix rooms, messages, members, profiles, and discovery. Two manage E2EE. The E2EE pair is not a second set of 23.

| Category | Tools |
|----------|-------|
| Rooms | list_joined_rooms, get_room_info, create_room, join_room, leave_room, list_invitations |
| Messages | get_room_messages, get_messages_by_date, send_message, send_direct_message |
| Members | get_room_members, invite_user, identify_active_users |
| Profiles | get_user_profile, get_my_profile, get_all_users |
| Admin | set_room_name, set_room_topic |
| Discovery | search_public_rooms, get_notification_counts, get_direct_messages |
| E2EE | bootstrap_e2ee, get_encryption_status |

`list_invitations` lists pending invites. It does not join or decline a room. With `BOT_ENABLED` unset it still works, and it reports trust as unknown.

When `BOT_ENABLED=true`, startup registers the message handler, starts the invite reconciler, then starts background sync. The reconciler joins a pending invite from a trusted sender and posts one greeting. A temporary join failure retries at most 5 times. A 403 that says the user is not invited yet is temporary while the invite remains. A ban, a missing room, an unsupported room version, and any other permanent failure stop after one attempt. The crate version in `Cargo.toml` is `0.4.0`.

## Tech Stack

| Component | Choice |
|-----------|--------|
| Language | Rust 2021 edition |
| MCP SDK | rmcp 1.2 (official Rust MCP SDK) |
| HTTP | axum + tower-http |
| Matrix SDK | matrix-sdk 0.19. A local build leaves `e2ee` off unless you pass `--features e2ee` |
| Serialization | serde + schemars |
| Error Handling | thiserror |
| Logging | tracing |
| E2EE | matrix-sdk-crypto. Release images are built with `--features e2ee`. The store opens only when `E2EE_ENABLED=true` |

## Quick Start

```bash
# Clone and configure
cp .env.example .env
# Edit .env with your Matrix homeserver URL, user ID, and access token

# Build and run. Release images also pass --features e2ee.
cargo build --features e2ee
cargo run --features e2ee

# The MCP endpoint is at http://localhost:3000/mcp
# Health check at http://localhost:3000/health
```

## Publishing (GHCR)

Official images are built by GitHub Actions and pushed to **GitHub Container Registry** when you push a **semver git tag** whose version matches [`Cargo.toml`](Cargo.toml) `version` (the same value baked into the binary as `CARGO_PKG_VERSION`).

1. Commit your changes and merge to the default branch.
2. Create an annotated tag with a `v` prefix. The current release is `0.4.0`:
   - `git tag -a v0.4.0 -m "matrix-mcp-server-r2 0.4.0"`
   - `git push origin v0.4.0`
3. Open the repo on GitHub → **Actions** and confirm the **Publish Docker image** workflow succeeds.
4. Pull the image:

```bash
docker pull ghcr.io/l0r3zz/matrix-mcp-server-r2:0.4.0
docker pull ghcr.io/l0r3zz/matrix-mcp-server-r2:latest
```

Release images and the release binary are built with `--features e2ee`. E2EE stays off at runtime until `E2EE_ENABLED=true`. The `:latest` tag is updated on each tag push that matches `v*` and has no pre-release suffix. The image tag `0.4.0` matches the server version reported by `GET /health` and `matrix-mcp-server-r2 --version`.

**Manual workflow run:** You can use **Actions → Publish Docker image → Run workflow** and select a **tag** as the ref (not only `main`) if you need to rebuild an existing release.

**Optional:** Create a **GitHub Release** from the same tag for release notes; it is not required for GHCR.

## Configuration

All configuration is via environment variables (or a `.env` file).
See `.env.example` for the full list.

| Variable | Description |
|----------|-------------|
| `MATRIX_HOMESERVER_URL` | Matrix homeserver URL (e.g., `https://matrix.example.com`) |
| `MATRIX_USER_ID` | Matrix user ID (e.g., `@bot:example.com`) |
| `MATRIX_ACCESS_TOKEN` | Matrix access token |
| `PORT` | Server port (default: 3000) |
| `E2EE_ENABLED` | Set to `true` to open the SQLite crypto store. The binary must be built with `--features e2ee` |
| `BOT_ENABLED` | Set to `true` to register the Matrix bot and the invite reconciler |
| `MODEL_ACCEPTS_IMAGES` | Set to `true` only when the Agent Zero model accepts images. Any other value refuses an image prompt |

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
  health.rs            # GET /health, including e2ee, crossSigned, and botEnabled
  lib.rs               # Library re-exports
  config.rs            # Configuration from environment variables
  error.rs             # Error types
  auth.rs              # Matrix auth context
  matrix/
    client.rs          # matrix-sdk client creation and background sync
    e2ee.rs            # Cross-signing bootstrap
    cache.rs           # TTL-based client cache
  mcp/
    server.rs          # MatrixMcpServer: 21 + 2 tools (23 total)
  bot/
    mod.rs             # Handler registration when BOT_ENABLED=true
    trigger.rs         # Prompt prefix and image decision
    dispatch.rs        # Fast replies, text prompts, and one image
    agent_zero.rs      # POST to the Agent Zero model
    invites.rs         # Trusted-invite reconciler
    trusted.rs         # Trusted-sender lookup
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
