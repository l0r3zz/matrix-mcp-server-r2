# Implementation Plan: Unified Cross-Signed E2EE MCP Server

**Branch**: `002-e2ee-cross-signing` | **Date**: 2026-10-29 | **Spec**: `.specify/specs/002-e2ee-cross-signing/spec.md`

## Summary

Upgrade `matrix-mcp-server-r2` to `matrix-sdk` 0.19 with a bundled SQLite crypto store, a per-agent `.device-id` aligned with `/_matrix/client/v3/account/whoami`, and automatic startup cross-signing bootstrap from a recovery secret. Upgrade the existing 20 MCP tools in place so external clients get Cross-Signed E2EE by default (`create-room` and `send-direct-message` default to `encryptionMode: "auto"` with `/keys/query` pre-flight fallback to unencrypted rooms; `send-message`, `get-room-messages`, `get-messages-by-date`, and `identify-active-users` transparently encrypt/decrypt), and add `bootstrap-e2ee` and `get-encryption-status`. `rmcp` stays at 1.2. Stage 2 (`.specify/specs/003-integrated-matrix-bot/spec.md`) will subsequently absorb `matrix-bot-rust` into `src/bot/` on top of this shared `matrix-sdk` 0.19 client.

## Technical Context

**Language/Version**: Rust >= 1.96, edition 2021
**Primary Dependencies**: `matrix-sdk` 0.19 (features `e2e-encryption`, `bundled-sqlite`, `rustls-tls`). `rmcp` 1.2. `ruma` as required by `matrix-sdk` 0.19
**Storage**: One SQLite directory per agent at `CRYPTO_STORE_PATH`, plus `.device-id` and an optional `0600` `recovery-key` file
**Testing**: `cargo test` without `e2ee`; `cargo test --features e2ee`; Continuwuity encrypted group-room and auto-fallback manual gate
**Target Platform**: Agent-Matrix container (`/a0/usr/workdir/matrix-mcp-server/`), Continuwuity homeserver
**Project Type**: Streamable HTTP MCP server (`/mcp` + `/health`)
**Performance Goals**: Startup auto-bootstrap under 60s excluding background key-backup download
**Constraints**: No TOFU inside encrypted rooms (`cross-signed` devices only). No raw secrets in tool I/O or logs. No hardcoded `MCP_SERVER` device id. CVE-2026-45056 fixed by `matrix-sdk` 0.19
**Scale/Scope**: One Matrix user and one E2EE device identity per agent container

## Constitution Check

- **Principle I (Unified Tool Surface)**: Existing 20 tool names remain stable and gain transparent E2EE + `auto` pre-flight fallback at room creation. No duplicate `-r2` room/message tools or `EncryptedRoomError` refusals.
- **Principle II (Feature-Gated E2EE)**: All E2EE paths gated behind `--features e2ee`; auto-bootstraps on startup when `E2EE_ENABLED=true`.
- **Principle IV (Matrix SDK as Crypto Authority)**: Crypto stays inside `matrix-sdk` 0.19 (`vodozemac` + bundled SQLite).
- **Principle V (Security First)**: Secrets stay off the tool channel and logs; Megolm keys go only to cross-signed devices.
- **Principle IX (Incremental Delivery)**: `002` delivers the unified E2EE MCP server; `003` absorbs `matrix-bot-rust` into `src/bot/`.

## Project Structure

### Documentation (this feature)

```text
.specify/specs/002-e2ee-cross-signing/
├── spec.md
├── plan.md
├── data-model.md
└── tasks.md
.specify/specs/003-integrated-matrix-bot/
└── spec.md                        # Stage 2 reactive bot absorption spec
spec/matrix-mcp-v1-v2.yml          # Unified 20+2 tool contract
spec/E2EE-Key-Management-Model.md  # Operator key model
```

### Source Code

```text
src/matrix/client.rs    # 0.19 builder, /whoami .device-id, SQLite store, auto-bootstrap, recipient strategy, sync
src/config.rs           # E2EE_ENABLED, CRYPTO_STORE_PATH, MATRIX_RECOVERY_KEY_FILE, optional UIAA password ref
src/mcp/server.rs       # 20 unified tools (transparent E2EE + auto pre-flight fallback) + bootstrap-e2ee + get-encryption-status
src/main.rs             # /health e2ee & crossSigned fields
Cargo.toml              # matrix-sdk 0.19 pin and rust-version 1.96
Dockerfile              # rust:1.96-bookworm builder image
```

**Structure Decision**: Single crate. Crypto setup and startup auto-bootstrap live in `src/matrix/client.rs` (or a `src/matrix/e2ee.rs` submodule gated by `#[cfg(feature = "e2ee")]`). MCP tool handlers remain in `src/mcp/server.rs`.

## Complexity Tracking

| Decision | Why Needed | Simpler Alternative Rejected Because |
|----------|------------|-------------------------------------|
| SDK major jump (`0.10` -> `0.19`) | `0.10` cannot meet the `matrix-sdk-crypto >= 0.16.1` floor (CVE-2026-45056) or 0.19 recipient trust APIs | Shipping E2EE on `0.10` leaves a known critical crypto vulnerability in the binary |
| Pre-flight `/keys/query` in `create-room` / `send-direct-message` (`encryptionMode: "auto"`) | Allows E2EE by default while automatically falling back to unencrypted rooms when an invitee lacks E2EE keys | Requiring callers to pass `-r2` tools or manual flags breaks zero-config E2EE for external clients |
