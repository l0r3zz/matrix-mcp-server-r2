# AGENTS.md -- Matrix MCP Server R2 (Rust)

Unified Rust Matrix gateway for the Agent-Matrix fleet. Exposes 20 operational Matrix MCP tools via Streamable HTTP transport (`/mcp`) that transparently support Cross-Signed E2EE by default (with automatic pre-flight `/keys/query` fallback to unencrypted rooms when an invitee lacks E2EE keys), plus 2 E2EE management tools (`bootstrap-e2ee` and `get-encryption-status`), and serves as the single Matrix client and SQLite crypto store owner per agent container (with Stage 2 `003-integrated-matrix-bot` absorbing `matrix-bot` into `src/bot/`).

## Stack

- **Language**: Rust (edition 2021, MSRV 1.99)
- **Async**: Tokio
- **HTTP**: Axum 0.8, Tower, tower-http (CORS, tracing)
- **MCP**: rmcp 1.2 (StreamableHttpService + LocalSessionManager at `/mcp`)
- **Matrix**: matrix-sdk 0.19 (`rustls-aws-lc-rs`; `e2e-encryption` and `bundled-sqlite` only behind the `e2ee` feature), ruma 0.17
- **Schemas**: schemars, serde, serde_json
- **Errors**: thiserror, anyhow

## Key Commands

```bash
# Build
cargo build                              # Debug
cargo build --release                    # Release (LTO + strip enabled)
cargo build --features e2ee              # Store opens only when E2EE_ENABLED=true

# Run
cargo run                                # Default: HTTP on port 3000
cargo run -- --version                   # Show version

# Test
cargo test                               # All tests
cargo test --test integration_tests      # Integration only

# Lint and format (CI denies warnings and requires rustfmt)
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features e2ee -- -D warnings
cargo fmt --all --check

# Docker (release images build with e2ee; --build-arg CARGO_FEATURES= to drop it)
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
  health.rs            -- GET /health JSON, including e2ee and crossSigned
  lib.rs
  config.rs            -- Config from env vars (OAuth/HTTPS/E2EE fields present but partial)
  error.rs
  auth.rs              -- MatrixAuthContext (minimal; from_config helper)
  matrix/
    client.rs          -- create_matrix_client, background sync loop with backoff
    e2ee.rs            -- cross-signing bootstrap (e2ee feature)
    secrets.rs         -- recovery-secret resolution
    cache.rs           -- TTL client cache (15-min TTL, periodic cleanup)
  mcp/
    server.rs          -- MatrixMcpServer: #[tool_router], 22 #[tool] methods
tests/
  config_tests.rs, cache_tests.rs, integration_tests.rs, health_tests.rs,
  server_tool_list_tests.rs, e2ee_tests.rs (`e2ee` feature)
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
- `E2EE_ENABLED` -- `true` requires `CRYPTO_STORE_PATH` and a build with `--features e2ee`
- `CRYPTO_STORE_PATH` -- SQLite store directory (mode `0700`). `.device-id` and `.store.lock` live here
- `MATRIX_RECOVERY_KEY_FILE` -- defaults to `$CRYPTO_STORE_PATH/recovery-key`. Contents are never logged
- `MATRIX_UIAA_PASSWORD_FILE` -- optional mode-0600 password file. Read only when `/keys/device_signing/upload` returns UIAA. Contents are never logged

OAuth/HTTPS env vars exist in config.rs but are partially implemented. The crypto store opens only when the `e2ee` feature and `E2EE_ENABLED=true` are both set. Recovery secrets resolve in order: `backupPassphraseRef` (errors until a secret-manager backend exists), `backupPassphraseEnvVar` (variable name, not the secret), then `MATRIX_RECOVERY_KEY_FILE`. A missing file means no secret.

## Architecture Decisions

- **Streamable HTTP MCP**: Uses rmcp's StreamableHttpService (not raw SSE), mounted at `/mcp` on Axum
- **Shared Matrix client**: Single client with background sync, NOT per-tool ephemeral clients
- **TTL client cache**: 15-min TTL with cleanup task; primed for multi-identity scenarios
- **SKIP_MATRIX_INIT**: Stub client for MCP Inspector / protocol testing without live Matrix
- **Background sync**: Loops `sync_once` with backoff so room state stays fresh. With the `e2ee` feature, the same loop processes to-device events and key updates through the SQLite store
- **Crypto store lock**: One exclusive `.store.lock` per `CRYPTO_STORE_PATH`. A second process gets `KeyStoreUnavailableError`
- **Recipient trust**: With the `e2ee` feature, room keys use `CollectStrategy::IdentityBasedStrategy` and decryption requires `TrustRequirement::CrossSigned`. Unverified devices are withheld with `m.unverified`
- **Bootstrap**: Startup runs `bootstrap_e2ee` when the store is open. The `bootstrap-e2ee` tool calls the same path. A new identity writes `recovery-key` mode `0600`. An existing identity with no secret returns `IdentityExistsError`. `forceRecreate` does not call `reset_cross_signing`. Key-backup download of joined rooms runs after the call returns (`keyBackupRestore`: `running`, then `done` or `failed`)
- **Room encryption**: `create-room` and `send-direct-message` take optional `encryptionMode` (`auto`, `e2ee`, `none`). When E2EE is active, omitted means `auto`: `/keys/query` runs before the room is created. All invitees cross-signed, or no invitees, sets `m.room.encryption` and history visibility `joined`. An invitee without keys falls back to an unencrypted room in `auto` (`Encrypted: No (fallback: @user:domain has no cross-signed E2EE keys)`) and returns `TrustPolicyNotSatisfiedError` in `e2ee`. A keys-query failure does not create the room. `none` stays unencrypted
- **Messages**: `send-message` reports `Encrypted: Yes` or `Encrypted: No`. Readers keep unable-to-decrypt events as `[Encrypted message - unable to decrypt (missing_key)]`. An encrypted send while E2EE is inactive returns `CryptoNotBootstrappedError`
- **Status**: `get-encryption-status` reports device, store, trust, and optional room encryption. `GET /health` always includes `e2ee` and `crossSigned`. Both are false when the `e2ee` feature, `E2EE_ENABLED`, or `SKIP_MATRIX_INIT` keeps crypto off. A binary without the feature returns `E2EE support not enabled; rebuild with --features e2ee` from the two E2EE tools
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
