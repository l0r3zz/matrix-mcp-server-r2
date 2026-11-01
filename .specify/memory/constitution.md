# matrix-mcp-server-r2 Constitution

`matrix-mcp-server-r2` is the unified Matrix gateway for each sovereign agent
in the Agent-Matrix fleet. It connects to the agent's Continuwuity homeserver
using a single `matrix-sdk` 0.19 client and a single persistent SQLite crypto
store per agent container, serving:

1. **Proactive MCP Tools (`/mcp`)**: A unified `20 + 2` tool surface where the
   20 existing tool names transparently support Cross-Signed E2EE by default
   (with automatic pre-flight `/keys/query` fallback to unencrypted rooms when
   an invitee lacks cross-signed E2EE keys), plus `bootstrap-e2ee` and
   `get-encryption-status`.
2. **Reactive Inbound Bot Dispatcher (`src/bot/`, feature `003-integrated-matrix-bot`)**:
   Absorbs `matrix-bot`'s reactive `/sync` event loop (trusted-invite
   auto-join, `@agent0-N` trigger filtering, `room_contexts.json`, and
   `POST /api/api_message` to Agent Zero) so all inbound and outbound Matrix
   traffic shares one device identity and crypto store.

E2EE trust is strictly **cross-signed only** (no `TOFU`). Active E2EE MCP spec
is `.specify/specs/002-e2ee-cross-signing/spec.md`; reactive bot absorption is
`.specify/specs/003-integrated-matrix-bot/spec.md`. The May spec
`001-e2ee-support` is superseded.

## Core Principles

### I. Unified Tool Surface & Seamless E2EE By Default

The 20 existing MCP tool names (`create-room`, `send-direct-message`,
`send-message`, `get-room-messages`, `get-messages-by-date`,
`identify-active-users`, etc.) MUST remain stable so external MCP clients
(Agent Zero, Cursor IDE, automation scripts) gain E2EE without changing tool
names or call patterns. Do not split tools into duplicate `-r2` variants or
reject encrypted rooms with `EncryptedRoomError`. When E2EE is enabled:
- `create-room` and `send-direct-message` default to `encryptionMode: "auto"`:
  they query `/_matrix/client/v3/keys/query` for all invitees and create an
  encrypted (`m.room.encryption`) room when all invitees have cross-signed E2EE
  keys (or no invitees are specified), or automatically fall back to an
  unencrypted room (reporting the fallback in the tool output) if an invitee
  has no cross-signed E2EE keys. Network or federation failures during
  `/keys/query` MUST fail closed (never downgrade on a transient error).
- `send-message`, `get-room-messages`, `get-messages-by-date`, and
  `identify-active-users` operate transparently on both encrypted and
  unencrypted rooms.

### II. Feature-Gated E2EE

All E2EE code paths MUST be gated behind the `e2ee` Cargo feature flag. The
server MUST build and run correctly without E2EE support when the feature is
not enabled. When built with `--features e2ee` and configured with
`E2EE_ENABLED=true`, the server MUST automatically bootstrap device keys and
cross-signing at startup. When E2EE is not compiled in, `bootstrap-e2ee` and
`get-encryption-status` MUST return a clear message indicating how to enable
the feature.

### III. Rust Idioms and Safety

Implementations SHOULD prefer borrowing over cloning. Any necessary
`.clone()` MUST be documented with a comment. Error types MUST use
`thiserror`. Production paths MUST NOT use `unwrap()` or `expect()`. Code
that holds a lock across `.await` MUST use `tokio::sync::Mutex`, not
`std::sync::Mutex`. All public functions MUST have doc comments. `unsafe`
code is forbidden unless it is absolutely required and thoroughly documented.

### IV. Matrix SDK as the Crypto Authority & Single Store Owner

Custom cryptography is forbidden. Encryption, decryption, key management,
and device verification MUST go through `matrix-sdk` and `matrix-sdk-crypto`
(`vodozemac`). Each agent container MUST have at most one process opening
`CRYPTO_STORE_PATH` and owning `.device-id`. Both MCP tool execution and
reactive bot dispatching (`src/bot/`) MUST share this single `matrix-sdk`
client instance rather than running parallel Matrix crypto clients.

### V. Security First

Encryption keys, session tokens, account passwords, and decrypted message
content MUST NOT be logged at INFO level or above. Crypto store paths MUST be
configurable via environment variables. Recovery keys MUST NOT be stored in
plaintext in container images, logs, or MCP tool results. Inside encrypted
rooms, Megolm room keys MUST go only to cross-signed devices (`TOFU` /
`ignore_unverified_devices` is forbidden). Unencrypted room support and
room-creation fallback operate strictly at the room state level
(`m.room.encryption`), never by weakening device trust inside encrypted rooms.
This software is built to be reliable and resilient as traffic to and from may pass
through an insecure and potentially hostile network. Consideration of any feature
implemented must keep "Security First" as a major guiding implementation principle.

### VI. Transparent Encryption

When E2EE is enabled, `matrix-sdk` 0.19 handles encryption and decryption
transparently via the shared client and SQLite store. Tool handlers and bot
sync handlers MUST handle encrypted and unencrypted rooms uniformly, surfacing
unable-to-decrypt (`UTD`) events gracefully with explicit status markers
rather than panicking or dropping messages silently.

### VII. Test Everything

New modules MUST have unit tests. Homeserver integration tests MUST use
`wiremock`. E2EE tests MUST be gated behind `#[cfg(feature = "e2ee")]`.
Both the feature-enabled build and the feature-disabled build MUST pass CI.

### VIII. Container-Aware Design

The server runs in Docker containers within the Agent-Matrix fleet. Design
choices MUST account for a persistent crypto store via volume mounts
(`/a0/usr/workdir/matrix-mcp-server/crypto-store`), stateless restarts with
session restoration, `.device-id` alignment with the access token's
`/_matrix/client/v3/account/whoami` device, and environment-variable-driven
configuration. The `/health` endpoint MUST always include `e2ee` and
`crossSigned` booleans, including when E2EE is not compiled in, when
`E2EE_ENABLED=false`, and when `SKIP_MATRIX_INIT=true` (both `false` in
those cases).

### IX. Incremental Delivery

Features MUST be delivered incrementally per the roadmap phases:
1. `002-e2ee-cross-signing`: `matrix-sdk` 0.19 migration, persistent SQLite
   crypto store, automatic startup cross-signing bootstrap, and Unified
   E2EE-By-Default MCP tools (`20 + 2`).
2. `003-integrated-matrix-bot`: Absorb `matrix-bot-rust` reactive `/sync` ->
   Agent Zero `/api/api_message` dispatch into `src/bot/` inside
   `matrix-mcp-server-r2`, retiring the separate `matrix-bot` process.
Each increment MUST be independently deployable and testable.

### X. Spec-Driven Development

New features MUST follow the Spec Kit workflow: specify, plan, tasks,
implement. Specs are the source of truth. `spec/matrix-mcp-v1-v2.yml` is
the API contract artifact. Spec Kit specs describe the broader feature
intent and acceptance criteria. Both artifacts MUST remain consistent.

## Locked Technology Stack

The versions below are constraints. A feature spec MUST NOT bump them.
Changing a locked version requires a constitution amendment.

| Component | Choice | Version |
|-----------|--------|---------|
| Language | Rust | 2021 edition. E2EE target MSRV 1.99 (current stable at amendment time; `matrix-sdk` 0.19 itself requires >= 1.96). Compiling pin is still 1.88 until the 0.19 migration |
| MCP SDK | rmcp | 1.2 (do not bump to 3.x as part of E2EE) |
| HTTP | axum + tower-http | 0.8 / 0.6 |
| Matrix SDK | matrix-sdk | E2EE target 0.19.x with `e2e-encryption` and `bundled-sqlite`. Compiling pin is still 0.10 |
| Crypto floor | matrix-sdk-crypto | >= 0.16.1 (included in 0.19; CVE-2026-45056) |
| Matrix types | ruma | Follows matrix-sdk 0.19. Do not keep an independent 0.12 pin after the bump |
| Serialization | serde + schemars | 1.0 / 1 |
| Error handling | thiserror | 2.0 |
| Logging | tracing + tracing-subscriber | 0.1 / 0.3 |
| Async runtime | tokio | 1.43 |
| E2EE | matrix-sdk with `e2e-encryption` + `bundled-sqlite` | via `e2ee` Cargo feature |

## Development Workflow

Code is organized as follows:

```text
src/
  main.rs              -- Entry point, axum/rmcp wiring, /health, graceful shutdown
  lib.rs               -- Library re-exports
  config.rs            -- Configuration from environment variables
  error.rs             -- Error types (thiserror)
  auth.rs              -- Per-request auth context
  matrix/
    client.rs          -- matrix-sdk 0.19 client, SQLite store, auto-bootstrap, sync loop
    cache.rs           -- TTL-based client cache
  mcp/
    server.rs          -- MatrixMcpServer: 20 unified tools + bootstrap-e2ee & get-encryption-status
  bot/                 -- (Stage 2 / 003) Reactive sync dispatcher to Agent Zero /api/api_message
spec/
  matrix-mcp-v1-v2.yml -- Unified API contract (20 operational tools + 2 E2EE management tools)
.specify/
  feature.json         -- Active Spec Kit feature directory pointer
  memory/              -- Constitution and project knowledge
  specs/               -- Feature specifications (002-e2ee-cross-signing, 003-integrated-matrix-bot)
  templates/           -- Spec Kit templates
```

Naming:

- MCP tools: `kebab-case` (for example, `list-joined-rooms`, `create-room`, `send-message`, `bootstrap-e2ee`, `get-encryption-status`). Do not add `-r2` suffixes.
- Tool JSON inputs: `camelCase` via `#[serde(rename = "...")]` (with `alias` for `snake_case` where helpful).
- Rust modules: `snake_case`
- Environment variables: `SCREAMING_SNAKE_CASE`
- Feature flags: `lowercase` (for example, `e2ee`)

Quality gates before a feature is done:

- `cargo test` passes with and without the `e2ee` feature when the change
  touches E2EE paths
- `cargo clippy` and `cargo fmt` are clean
- The feature spec, plan, and tasks agree with this constitution
- `spec/matrix-mcp-v1-v2.yml` stays consistent with any tool-contract change

References:

- Parent project: Agent-Matrix sovereign agent fleet
- API contract: `spec/matrix-mcp-v1-v2.yml`
- Cursor rules: `.cursor/rules/rust-e2ee.mdc`
- Active E2EE MCP spec: `.specify/specs/002-e2ee-cross-signing/spec.md`
- Reactive bot integration spec: `.specify/specs/003-integrated-matrix-bot/spec.md`
- Architecture & tradeoff analyses:
  - `spec/E2EE-Repository-Audit-20261029.md`
  - `spec/Agent-Matrix-E2EE-Context-Synthesis-20261029.md`
  - `spec/E2EE-Unified-vs-TwoTier-API-Tradeoff-20261029.md`
  - `spec/Matrix-Bot-vs-MCP-Server-Consolidation-Analysis-20261029.md`
  - `spec/E2EE-Landscape-Review-October-20261023.md`
  - `spec/E2EE-Key-Management-Model.md`

## Governance

This constitution supersedes conflicting local practice for
`matrix-mcp-server-r2`. Feature specs, plans, and tasks MUST comply with it.
Runtime development guidance lives in `.cursor/rules/rust-e2ee.mdc` and the
API contract in `spec/matrix-mcp-v1-v2.yml`.

Amendments are edits to this file. Each amendment MUST bump the version and
update Last Amended.

Versioning:

- MAJOR: a principle is removed or redefined in a way existing specs cannot
  satisfy unchanged
- MINOR: a principle or section is added, or guidance is materially expanded
- PATCH: clarification, wording, or non-semantic fixes

Compliance review: a change that touches tool contracts, crypto, or the
`e2ee` feature MUST be checked against Principles I, II, IV, V, and VI before
implementation. A locked stack version MUST NOT change inside a feature
spec. That change requires an amendment here first.

**Version**: 2.0.2 | **Ratified**: 2026-05-11 | **Last Amended**: 2026-10-31
