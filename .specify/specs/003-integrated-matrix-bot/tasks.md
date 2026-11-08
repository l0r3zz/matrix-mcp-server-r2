# Tasks: Integrated Reactive Matrix Bot Dispatcher (`src/bot/`)

**Input**: `.specify/specs/003-integrated-matrix-bot/`
**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/bot-dispatch.md`, `quickstart.md`

The spec has no numbered user stories. These tasks use the three success criteria as the stories:

- **US1 (P1)**: SC-001 fast commands
- **US2 (P2)**: SC-002 Agent Zero reply, one poster
- **US3 (P3)**: SC-003 trusted invites

Unit tests are included because Constitution Principle VII and `plan.md` require them for `src/bot/`. Write each story's tests so they fail before that story's implementation.

No task hardcodes an instance number. Prefixes come from the process Matrix user id (`@agent0-N`). The only concrete lab agent is `@agent0-5`.

## Phase 1: Setup

**Purpose**: Add the module and the one new library. Do not change the 20+2 MCP tools.

- [X] T001 Add `pulldown-cmark` to `Cargo.toml`. Do not change the `matrix-sdk` 0.19 pin, `rmcp` 1.2, or `rust-version` 1.99.
- [X] T002 Create `src/bot/mod.rs` and add `pub mod bot;` in `src/lib.rs`. The module compiles with and without `--features e2ee`. It does not construct a `matrix-sdk` client and does not open `CRYPTO_STORE_PATH`.

## Phase 2: Foundational

**Purpose**: Configuration, health, and on-disk state that every story uses. No story starts before this phase.

- [X] T003 Add the bot fields to `src/config.rs` with these defaults from `data-model.md`: `BOT_ENABLED` default `false` (`true` registers handlers; any other value leaves them off); `BOT_DISPLAY_NAME` default empty; `AGENT_IDENTITY` default `BOT_DISPLAY_NAME`, then `Agent Zero`; `A0_API_URL` default `http://localhost:80/api/api_message`; `AMCTL_URL` default `http://172.23.88.254:8077`; `AMCTL_API_KEY` unset; `TRUSTED_SENDERS` unset; `BOT_STATE_DIR` default `/a0/usr/workdir/matrix-bot`. Never log `AMCTL_API_KEY`, `.a0-api-key`, or the Matrix access token.
- [X] T004 [P] Add `botEnabled` (boolean) to `GET /health` in `src/health.rs`. It is `true` only when `BOT_ENABLED=true` and the handlers were registered. Leave `e2ee` and `crossSigned` as feature 002 defined them.
- [X] T005 [P] Implement load and save in `src/bot/state.rs`. `room_contexts.json` is a JSON object of `room_id` to Agent Zero `context_id`. A missing file is an empty map. A room with no entry sends `context_id` as an empty string. `processed_event_ids.json` is a JSON array of event ids, newest at the end, length at most 1000. Do not read or write `sync_cursor.json`.
- [X] T006 In `src/main.rs`, pass the existing `Client` from `src/matrix/client.rs` into `src/bot/` only when `BOT_ENABLED=true`. When `BOT_DISPLAY_NAME` is non-empty, set that display name on the shared client at startup, before handlers are registered. Do not register handlers when the flag is false. Do not add a second `/sync` loop. Do not open `CRYPTO_STORE_PATH`. Do not call logout or reset a password. Do not stop `run_bot`.

**Checkpoint**: With `BOT_ENABLED` unset, the process behaves as feature 002 and `GET /health` reports `botEnabled: false`.

## Phase 3: User Story 1 - Fast commands (Priority: P1)

**Goal**: `@agent0-N: ping` and `@agent0-N: hello` get one immediate reply from this process, in an encrypted room and in an unencrypted room, without calling Agent Zero (SC-001).

**Independent Test**: Unit tests for a fixture user `@agent0-5:example.com` and a second user `@other:example.com`. Each user's prefixes come from that user id. `ping` returns `PONG` and does not call Agent Zero.

### Tests for User Story 1

- [X] T007 [P] [US1] Add unit tests in `src/bot/trigger.rs` for prefix match, mention-pill stripping, case-insensitive match, the 1000-id cap, stale drop when `now_ms - origin_server_ts > 300000`, self-sender drop, non-`m.text` drop, and an unable-to-decrypt result that carries the event id and reason and no message body. Use `@agent0-5:example.com` and `@other:example.com`. Do not hardcode an instance number in the matcher.

### Implementation for User Story 1

- [X] T008 [US1] Implement prefix building in `src/bot/trigger.rs` from the session user id localpart `agent_short` and optional `BOT_DISPLAY_NAME`. The prefixes are `{agent_short}:`, `{agent_short},`, `@{agent_short}`, `@all-agents:`, `@all-agents,`, plus `{display_name}:`, `{display_name},`, and `@{display_name}` when `BOT_DISPLAY_NAME` is non-empty and is not the same string as `agent_short`. A prefix is required in every room, including a two-member room.
- [X] T009 [US1] Implement the decision order in `src/bot/dispatch.rs`: (1) self, `sender` equals the session user id, record the event id, no reply; (2) duplicate event id, no reply; (3) stale, `now_ms - origin_server_ts > 300000`, no reply; (4) not `m.room.message` or `msgtype` is not `m.text`, no reply; (5) decryption failed, structured warning with event id and reason, no message body, no room post; (6) no trigger, no reply; (7) fast command. Record the event id for each classified event, including events with no reply, and drop from the front past 1000 entries. Fast replies, with no Agent Zero call, are `ping` -> `PONG`, `hello` -> `Hello! I'm {identity}, online and ready.`, `whoami` -> `I'm {identity} ({user_id}).` `{identity}` is `AGENT_IDENTITY`.
- [X] T010 [US1] Register the message handler on the shared `Client` in `src/bot/mod.rs` before `start_background_sync` in `src/main.rs`. Encrypted-event decryption is behind `#[cfg(feature = "e2ee")]`. With the `e2ee` feature off, do not dispatch encrypted events.

**Checkpoint**: Fast commands reply once. A non-command trigger is recognized and not sent anywhere yet.

## Phase 4: User Story 2 - Agent Zero reply (Priority: P2)

**Goal**: `@agent0-N:` plus a question reaches `/api/api_message`, keeps `room_contexts.json`, and produces one HTML reply posted only by `src/bot/` (SC-002).

**Independent Test**: A wiremock `POST` returns a context id. The room map gains that entry. A 401 causes one re-read of `.a0-api-key` and one retry. The test does not perform a second room send.

### Tests for User Story 2

- [X] T011 [P] [US2] Add a wiremock test in `tests/bot_agent_zero.rs` for `POST {A0_API_URL}` with JSON `message` and `context_id`. Cover an empty `context_id` on the first turn, a returned context id stored for that `room_id`, HTTP 401 followed by one key re-read and one retry, and a failed POST that produces no reply body.

### Implementation for User Story 2

- [X] T012 [US2] Implement the Agent Zero call in `src/bot/agent_zero.rs`. Read the credential from `{BOT_STATE_DIR}/.a0-api-key`. On HTTP 401, re-read the file once and retry the POST once. Send header `X-API-KEY` and `Content-Type: application/json`. The `message` field starts with the preface that tells Agent Zero to reply in plain conversational text and not to send messages via Matrix MCP tools, then the sender and the room id, then the triggered text. Never log the key. `AgentZeroAuth` logs the status only and sends no partial reply. `AgentZeroUnavailable` sends no fabricated answer.
- [X] T013 [US2] Spawn that HTTP call on its own task from `src/bot/dispatch.rs`. Set typing `true` with timeout `60000` before the call and typing `false` after the send or the error. Do not await the call on the sync task.
- [X] T014 [US2] Render the reply with `pulldown-cmark` and send it with `room.send()` from `src/bot/dispatch.rs`. Chunks are at most 4000 characters of the plain text, split on a newline when one exists in the last 500 characters of the chunk. The send carries the plain text and the HTML. In an encrypted room the SDK encrypts. Update `room_contexts.json` when the returned context id changes. No other code path posts this reply.

**Checkpoint**: A question produces one bot reply and updates the context file. Fast commands still skip Agent Zero.

## Phase 5: User Story 3 - Trusted invites (Priority: P3)

**Goal**: A trusted invite joins and greets once. An untrusted invite, or a failed lookup with an empty fallback, does not join (SC-003).

**Independent Test**: A failed am-ctl response with empty `TRUSTED_SENDERS` does not join. A non-empty env list is used only when the in-memory set is empty. A non-empty am-ctl array replaces the set.

### Tests for User Story 3

- [X] T015 [P] [US3] Add a wiremock test in `tests/bot_trusted.rs` for `GET {AMCTL_URL}/api/v1/trusted-senders`. Cover a non-empty JSON array of objects with `matrix_user_id` replacing the set, a failed GET with empty `TRUSTED_SENDERS` that does not join, and a failed GET that uses a non-empty `TRUSTED_SENDERS` list only when the in-memory set is empty. Assert the binary contains no compiled Matrix user id and no compiled API key. The tests name no instance other than the `@agent0-5` lab id when a sample invitee is required.

### Implementation for User Story 3

- [X] T016 [US3] Implement invite handling in `src/bot/trusted.rs`. `GET {AMCTL_URL}/api/v1/trusted-senders` with header `X-API-Key` from `AMCTL_API_KEY` and a 5 second timeout. A successful non-empty JSON array of objects with `matrix_user_id` replaces the in-memory set. If the call fails or the set is empty, use `TRUSTED_SENDERS` (comma-separated Matrix ids) only when the in-memory set is still empty. If that list is also empty, reject the invite, log the room id and the sender, and do not join. Join only when the inviter is in the set. After a successful join, send once: `Hello! I'm {display_name}, an AI agent powered by Agent Zero.`

**Checkpoint**: Trusted join greets once. Empty trust state does not join.

## Phase 6: Polish

**Purpose**: Both feature builds stay green, and the lab proof names `@agent0-5` only.

- [X] T017 Run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features e2ee -- -D warnings`, `cargo test`, and `cargo test --features e2ee`. Confirm `spec/matrix-mcp-v1-v2.yml` has no tool added or renamed.
- [X] T018 Run `.specify/specs/003-integrated-matrix-bot/quickstart.md` on `@agent0-5` after `run_bot` is stopped and `BOT_ENABLED=true`. The step-by-step lab is `spec/T018-Agent0-5-Test-Plan-CLEAR-20261106.md`. Record the result in `spec/` with a dated filename. Do not use another agent as the lab target. The trusted invite passed in T019. The untrusted invite passed on 2026-11-08. The record is `spec/T018-Agent0-5-Result-20261107.md`.
- [X] T019 Upgrade Synapse on `matrix.v-site.net` from `1.147.1` to image `matrixdotorg/synapse:v1.164.2` by changing the `matrix` container in Deployment `matrix-synapse` and letting the Deployment roll. Leave Continuwuity and the Helm chart version. The plan is `spec/Synapse-Upgrade-Plan-20261107.md`. Then repeat the trusted invite in the T018 lab. Passed 2026-11-08 on image `ghcr.io/element-hq/synapse:v1.164.2`. The record is `spec/T019-Agent0-5-Result-20261107.md`.

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies.
- **Foundational (Phase 2)**: Depends on T002. Blocks US1, US2, and US3.
- **US1 (Phase 3)**: Depends on Phase 2. No dependency on US2 or US3.
- **US2 (Phase 4)**: Depends on Phase 2 and on the US1 decision order in T009. Fast commands must keep skipping Agent Zero.
- **US3 (Phase 5)**: Depends on Phase 2. No dependency on US2.
- **Polish (Phase 6)**: Depends on the stories being implemented.

### User Story Dependencies

- **US1 (P1)**: First implementable story. MVP.
- **US2 (P2)**: Uses the US1 classifier, then adds the Agent Zero path.
- **US3 (P3)**: Invite path only. Can proceed beside US2 after Phase 2.

### Parallel Opportunities

- T001 and T002 touch different files and can run together.
- T004 and T005 touch different files and can run together after T003.
- T007, T011, and T015 are tests in different files and can be written in parallel after Phase 2.
- T016 can proceed beside T012 once Phase 2 and the handler registration in T010 exist.

## Parallel Example: User Story 1

```text
T007 unit tests in src/bot/trigger.rs
```

T008 and T009 stay sequential in that file and in `src/bot/dispatch.rs`.

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1 and Phase 2.
2. Complete Phase 3.
3. Stop. `ping` and `hello` reply once for a derived `@agent0-N` prefix, with `BOT_ENABLED=true` and `run_bot` stopped.

### Incremental Delivery

1. US1 delivers SC-001.
2. US2 delivers SC-002 without a second poster.
3. US3 delivers SC-003.
4. T018 is the lab proof on `@agent0-5`. T019 is the Synapse image upgrade that lets the trusted invite reach the bot.

## Notes

- [P] means a different file and no unfinished dependency.
- Story labels apply only to Phases 3 through 5.
- Do not copy `unwrap()` or `expect()` from `matrix-bot-rust`. Use `thiserror` on library errors.
- A lock held across `.await` uses `tokio::sync::Mutex`.
