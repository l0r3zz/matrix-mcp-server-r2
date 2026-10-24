# Implementation Plan: Cross-Signed E2EE

**Branch**: `002-e2ee-cross-signing` | **Date**: 2026-10-23 | **Spec**: `.specify/specs/002-e2ee-cross-signing/spec.md`

## Summary

Bump the Matrix client to `matrix-sdk` 0.19 with a bundled SQLite store and a per-agent device id, then bootstrap cross-signing from a recovery secret. v2 tools encrypt only for cross-signed devices. `rmcp` stays at 1.2. The May spec is not an input to implementation.

## Technical Context

**Language/Version**: Rust >= 1.96, edition 2021
**Primary Dependencies**: `matrix-sdk` 0.19 (features `e2e-encryption`, `bundled-sqlite`, rustls). `rmcp` 1.2. `ruma` as required by matrix-sdk 0.19
**Storage**: One SQLite directory per agent at `CRYPTO_STORE_PATH`, plus `.device-id` and an optional 0600 recovery file
**Testing**: `cargo test` without `e2ee`; `cargo test --features e2ee`; one Continuwuity group-room manual gate
**Target Platform**: Agent-Matrix container, Continuwuity homeserver
**Project Type**: HTTP MCP server
**Performance Goals**: Bootstrap under 60s excluding a full key-backup download
**Constraints**: No TOFU. No raw secrets in tool I/O or logs. No `MCP_SERVER` device id. CVE-2026-45056 fixed by being on 0.19
**Scale/Scope**: One Matrix user and one device per agent process

## Constitution Check

- v1 tool names and unencrypted behavior stay.
- Crypto stays inside `matrix-sdk`.
- E2EE is feature-gated.
- Secrets stay off the tool channel.
- `rmcp` 3.x is a different project.

## Project Structure

### Documentation (this feature)

```text
.specify/specs/002-e2ee-cross-signing/
├── spec.md
├── plan.md
├── data-model.md
└── tasks.md
spec/matrix-mcp-v1-v2.yml          # tool contract
spec/E2EE-Key-Management-Model.md  # operator model
```

### Source Code

```text
src/matrix/client.rs    # builder, device id, store, sync
src/config.rs           # CRYPTO_STORE_PATH, recovery source
src/mcp/server.rs       # -r2 tools and v1 encrypted-room refusal
src/main.rs             # health fields
Cargo.toml              # 0.19 pin and rust-version 1.96
```

**Structure Decision**: No new crate. Crypto setup lives next to the existing client constructor. Tools stay in `mcp/server.rs` until that file becomes unreadable; splitting is allowed in the task that adds the second `-r2` tool if the file exceeds a reviewable size.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| SDK major jump inside an E2EE feature | 0.10 cannot meet the crypto floor or the 0.19 trust APIs | Shipping E2EE on 0.10 leaves CVE-2026-45056 in the binary |
| Feature flag kept after 0.19 makes encryption a default feature | Fleet images must still build a non-crypto binary | `default-features = false` preserves that |
