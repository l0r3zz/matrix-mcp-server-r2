# Implementation Plan: Integrated Reactive Matrix Bot Dispatcher (`src/bot/`)

**Branch**: `003-integrated-matrix-bot` | **Date**: 2026-11-05 | **Spec**: `.specify/specs/003-integrated-matrix-bot/spec.md`

## Summary

Absorb the reactive work of `matrix-bot-rust` into `src/bot/` inside the existing `matrix-mcp-server-r2` process. One agent keeps one Matrix device: the device already bound to `CRYPTO_STORE_PATH` and `.device-id`. MCP tools and the bot share that `matrix-sdk` 0.19 client, that access token, that SQLite store, and the existing `sync_once` loop. `BOT_ENABLED=true` turns the dispatcher on. `supervisord` program `run_bot` is stopped before that flag is set. The 20+2 MCP tool surface does not change.

## Technical Context

**Language/Version**: Rust >= 1.99, edition 2021
**Primary Dependencies**: Existing `matrix-sdk` 0.19, `rmcp` 1.2, `ruma` as pulled by `matrix-sdk` 0.19, `tokio` 1.x. Add `pulldown-cmark` for the Markdown-to-HTML reply the spec already requires.
**Storage**: The 002 SQLite crypto store stays the only Matrix store. Bot state is two JSON files beside the current bot workdir: `room_contexts.json` (kept in place) and `processed_event_ids.json` (new). Do not read or write `sync_cursor.json`.
**Testing**: `cargo test` and `cargo test --features e2ee`. Unit tests for trigger parsing, dedup, stale drop, fast commands, and trusted-sender fail-closed behavior. Wiremock for the am-ctl trusted-senders HTTP call and the Agent Zero `/api/api_message` call. Encrypted-room proof stays a lab check in `quickstart.md`.
**Target Platform**: Agent-Matrix container. Binary at `/a0/usr/workdir/matrix-mcp-server/`. Bot state at `/a0/usr/workdir/matrix-bot/`.
**Project Type**: The same Axum process: Streamable HTTP MCP at `/mcp`, `GET /health`, plus an in-process Matrix event dispatcher.
**Performance Goals**: `ping`, `hello`, and `whoami` reply without calling Agent Zero. An Agent Zero HTTP call must not run on the sync task.
**Constraints**: One device per agent. Cross-signed only. No TOFU. No second opener of `CRYPTO_STORE_PATH`. No `--logout` and no password reset. `history_visibility` stays `invited`. No new MCP tool names and no `-r2` split. No secrets in logs. `BOT_ENABLED` defaults to false.
**Scale/Scope**: One dispatcher per agent container. Five lab agents. Trigger grammar and fast commands match the live `matrix-bot-rust` behavior cited in `research.md`.

## Constitution Check

- **Principle I (Unified Tool Surface)**: No tool names, schemas, or `encryptionMode` behavior change. `spec/matrix-mcp-v1-v2.yml` stays as it is.
- **Principle II (Feature-Gated E2EE)**: `src/bot/` compiles in both feature builds. Decryption of `m.room.encrypted` exists only behind `e2ee`. With the feature off, encrypted events are not dispatched.
- **Principle III (Rust Idioms)**: The port does not copy `unwrap()` / `expect()` from `matrix-bot-rust`. Errors use `thiserror`. A lock held across `.await` uses `tokio::sync::Mutex`.
- **Principle IV (Single Store Owner)**: `src/bot/` receives the `Client` created in `src/matrix/client.rs`. It does not build a second client or open `CRYPTO_STORE_PATH`.
- **Principle V (Security First)**: `AMCTL_API_KEY`, `.a0-api-key`, and the Matrix access token are read from the environment or the existing secret file and are never logged. No baked-in API key.
- **Principle VI (Transparent Encryption and UTD)**: Encrypted and unencrypted rooms use the same trigger path after the SDK yields plaintext. An unable-to-decrypt event emits a structured warning with the event id and reason, and no reply is sent. MCP readers keep the existing `[Encrypted message - unable to decrypt (...)]` marker.
- **Principle VII (Test Everything)**: New `src/bot/` modules have unit tests. CI already runs both feature builds.
- **Principle VIII (Container-Aware Design)**: `GET /health` gains `botEnabled` (boolean). `e2ee` and `crossSigned` stay as 002 defined them. `.device-id` is still aligned with `/whoami`.
- **Principle IX (Incremental Delivery)**: 002 stays deployed. 003 is off until `run_bot` is stopped and `BOT_ENABLED=true`.
- **Principle X (Spec-Driven)**: This plan, `research.md`, `data-model.md`, `contracts/bot-dispatch.md`, `quickstart.md`, and `tasks.md` are the design. No `src/bot/` code until implementation is requested.

Post-design re-check: the same gates pass. The contract adds no MCP tool. The data model adds files outside the crypto store. UTD is explicit and does not panic or vanish into a silent drop.

## Project Structure

### Documentation (this feature)

```text
.specify/specs/003-integrated-matrix-bot/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── bot-dispatch.md
└── tasks.md                 # not created by this plan
```

### Source Code

```text
src/bot/                    # new module, not created in this plan
  mod.rs                    # start when BOT_ENABLED=true
  dispatch.rs               # trigger, dedup, fast commands, Agent Zero call
  trusted.rs                # am-ctl trusted-senders lookup
src/matrix/client.rs        # existing sync_once loop; bot registers handlers on this Client
src/config.rs               # BOT_ENABLED and the bot env vars
src/health.rs               # botEnabled field
src/main.rs                 # start the dispatcher after the client is ready
```

**Structure Decision**: One crate. The bot is a module beside `src/mcp/` and `src/matrix/`. It does not get its own binary.

## Complexity Tracking

| Decision | Why Needed | Simpler Alternative Rejected Because |
|----------|------------|-------------------------------------|
| Keep the existing crypto-store device | The user required one device per agent. That store is already cross-signed | A new device, or keeping AgentZeroBot, leaves a second device or breaks 002 sessions |
| `BOT_ENABLED` defaults false | `run_bot` still answers until it is stopped | Defaulting on makes both processes reply to the same trigger |
| Agent Zero work on a spawned task | The sync loop also delivers to-device key updates | Awaiting `/api/api_message` inside `sync_once` stalls Olm sessions |
| Leave `sync_cursor.json` unread | `matrix-sdk` already stores `next_batch` | A second cursor replays or skips events the SDK has already applied |
