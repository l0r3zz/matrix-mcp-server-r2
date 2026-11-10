# Tasks: Invite Reconciliation and Discovery

**Input**: `.specify/specs/004-invite-reconciliation/`
**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/list-invitations.md`, `quickstart.md`

Stories come from `spec.md`:

- **US1 (P1)**: Join trusted invites that arrived while the agent was down
- **US2 (P1)**: List pending invites (`list_invitations`)
- **US3 (P2)**: Join trusted invites while running, with bounded retry

Tests are included because Constitution Principle VII requires them. Write each story's tests so they fail before that story's implementation.

No task hardcodes an instance number. Fixtures use `@agent0-5:example.com` and `@other:example.com`. The canary is `@agent0-2` (user choice for 004).

Do not add a Cargo dependency. Do not open a second crypto store or start a second sync loop. Do not log tokens, recovery keys, `AMCTL_API_KEY`, or server error bodies.

## Phase 1: Setup

**Purpose**: Governance and contract updates, and the empty module.

- [ ] T001 Amend `.specify/memory/constitution.md` to v2.1.0 (MINOR), and add a Sync Impact Report entry:
  - Change `20 + 2` to `21 + 2` on lines 8 and 118.
  - Add `004-invite-reconciliation` to the Principle IX roadmap.
  - Change the naming line (line 178) from `kebab-case` to `snake_case`, with the examples `list_joined_rooms`, `join_room`, `bootstrap_e2ee`, `get_encryption_status`, and `list_invitations`.
  - Do not change the locked stack table.
- [ ] T002 [P] Add `list_invitations` to `spec/matrix-mcp-v1-v2.yml` as a read (tier0) tool, with no input and the output from `contracts/list-invitations.md`. Change the tool count to `21 + 2`.
- [ ] T003 [P] Create `src/bot/invites.rs` with a module doc comment, and add `pub mod invites;` in `src/bot/mod.rs`. It compiles with and without `--features e2ee`.

## Phase 2: Foundational

**Purpose**: Shared state and wiring that every story uses. Behavior does not change in this phase.

- [ ] T004 Define the types from `data-model.md` in `src/bot/invites.rs`:
  - `InviteRegistry` holds `tokio::sync::Mutex<HashMap<OwnedRoomId, InviteEntry>>` and `active: AtomicBool`. Its methods are `new()`, `is_active()`, `prune(&[OwnedRoomId])`, and `snapshot()`.
  - `InviteEntry` has `inviter: Option<OwnedUserId>`, `last_decision: Option<Decision>`, `attempts: u8`, `last_error: Option<JoinError>`, `next_attempt_at: Option<tokio::time::Instant>`, and `in_flight: bool`.
  - `Decision` is `Join`, `SkipUntrusted`, or `SkipNoInviter`.
  - `ErrorClass` is `Permanent` or `Temporary`.
  - `JoinError` has `class`, `status: Option<u16>`, and `code: Option<String>`. Its `Display` output is `"<class> <status> <code>"`, for example `permanent 403 M_FORBIDDEN`. It "never holds the server error message body".
- [ ] T005 Change `start_background_sync` in `src/matrix/client.rs` to `start_background_sync(client, after_sync: Option<Arc<tokio::sync::Notify>>)`. After each `Ok` from `sync_once`, call `notify_one()`. The error path and the sleeps stay the same.
- [ ] T006 [P] Add `MatrixMcpServer::with_invites(self, Arc<InviteRegistry>) -> Self` in `src/mcp/server.rs`. Store an `Arc<InviteRegistry>` field, and default it to an empty, inactive registry in `new()`. Keep the signature of `MatrixMcpServer::new(client, config)`.
- [ ] T007 In `src/main.rs`, create one `Arc<InviteRegistry>` and one `Arc<Notify>` whether `BOT_ENABLED` is set or not. Pass the registry to the MCP service factory with `with_invites`. Pass `Some(notify)` to `start_background_sync`.

**Checkpoint**: `cargo test` and `cargo test --features e2ee` pass. Behavior is the same as `v0.3.1`.

## Phase 3: User Story 1 - Offline trusted invite (Priority: P1)

**Goal**: An invite that arrived while the service was stopped is joined within 60 seconds after healthy, with one greeting (SC-001).

**Independent Test**: A wiremock `/sync` response returns `rooms.invite` for a trusted inviter before the reconciler starts. One pass makes exactly one join request and one greeting send.

### Tests for User Story 1

- [ ] T008 [P] [US1] Add unit tests in `src/bot/invites.rs` for `decide()`:
  - An exact full user id in the trusted set gives `Join`.
  - A display name, a localpart only, or a domain-only match gives `SkipUntrusted` (FR-004).
  - An inviter of `None` gives `SkipNoInviter` (FR-005).
  - An empty set gives `SkipUntrusted`.
  - `should_log(prev, new)` is true only when the decision changes (FR-014).
- [ ] T009 [P] [US1] Add `tests/bot_invites.rs` with wiremock. Use a `/_matrix/client/v3/sync` response whose `rooms.invite` has a stripped `m.room.member` event with `membership: invite`, `state_key` set to the session user, and `sender` set to the inviter. Cover these cases:
  - A trusted inviter gives exactly one `POST /_matrix/client/v3/join/{roomId}` and one greeting `PUT .../send/m.room.message/...`.
  - An untrusted inviter gives zero join requests.
  - No invites give zero `GET /api/v1/trusted-senders` requests (FR-003).
  - A trusted-sender lookup that returns 500, with an empty `TRUSTED_SENDERS`, gives zero join requests.

### Implementation for User Story 1

- [ ] T010 [US1] Implement `decide(inviter: Option<&UserId>, trust: &TrustMemory) -> Decision` in `src/bot/invites.rs`, using `TrustMemory::admits` on the full user id string.
- [ ] T011 [US1] Implement `reconcile_pass(client, runtime, registry)` in `src/bot/invites.rs`, one attempt per room for now. The steps are:
  1. Read `client.invited_rooms()`, and prune entries for rooms that are no longer invited (FR-016).
  2. Return with no am-ctl call if the list is empty.
  3. Call `trusted::refresh` once.
  4. For each room, read `invite_details().inviter_id`. An error gives `None`.
  5. Decide the room, and log `room_id`, `inviter`, and `decision` only when the decision changes.
  6. On `Join`, call `room.join()` without holding the registry lock.
  7. On `Ok`, remove the entry and send `trusted::greeting(display_name)` once. A greeting failure logs `warn` and is not retried.
- [ ] T012 [US1] In `src/bot/mod.rs`:
  - Remove the join path from `register_invite_handler`. Remove the handler if it has no other duty.
  - Stop writing `invite:{room}:{sender}` entries to `processed_event_ids.json`.
  - Make `attach` return `Option<Arc<Runtime>>`, which is `Some` only when the handlers were registered.
  - Keep `trusted::invite_action` and its tests in `tests/bot_trusted.rs`.
- [ ] T013 [US1] Add `spawn_reconciler(client, runtime, registry, notify) -> JoinHandle<()>` in `src/bot/invites.rs`. It sets `registry.active`, runs one pass at once (FR-001), then runs one pass per `notify.notified()` (FR-002). In `src/main.rs`, call it after `bot::attach` returns `Some` and before `start_background_sync`. Keep the handle, and abort it with `sync_handle` and `cleanup_handle` in the graceful shutdown block (FR-017).

**Checkpoint**: The T009 tests pass. A trusted invite in the first sync is joined. An untrusted invite is not joined.

## Phase 4: User Story 2 - `list_invitations` (Priority: P1)

**Goal**: An agent or an operator can see every pending invite, with the trust result and the last error (FR-010 to FR-013).

**Independent Test**: With two invites in `/sync`, `list_invitations` returns both, sorted by `roomId`. With the registry inactive, `trusted` is `"unknown"` and `attempts` is 0.

### Tests for User Story 2

- [ ] T014 [P] [US2] Add unit tests in `src/bot/invites.rs` for `render_invitations`:
  - Output is sorted by `roomId`.
  - At most 100 entries are returned, with `truncated: true` when more exist.
  - An empty input gives `{"count":0,"truncated":false,"invitations":[]}`.
  - `trusted` is `"true"`, `"false"`, or `"unknown"`, and `encryption` is `"encrypted"`, `"unencrypted"`, or `"unknown"`.
- [ ] T015 [P] [US2] Add a wiremock test in `tests/bot_invites.rs` that calls `MatrixMcpServer::list_invitations` after one `/sync` with two invites:
  - Check `roomId`, `name`, `canonicalAlias`, `topic`, `inviter`, and `encryption`.
  - Check `trusted: "unknown"` and `attempts: 0` with an inactive registry (FR-011).
  - Check that the output does not contain the test access token string (FR-012).
- [ ] T016 [P] [US2] In `tests/server_tool_list_tests.rs`, assert that the tool router lists `list_invitations`, and that the count is one more than `v0.3.1` in each feature build.

### Implementation for User Story 2

- [ ] T017 [US2] Add `InvitationView` and `render_invitations(Vec<InvitationView>) -> serde_json::Value` in `src/bot/invites.rs`, following the field table in `contracts/list-invitations.md`. `lastError` uses the `JoinError` display, and "never a server message body".
- [ ] T018 [US2] Add the `#[tool]` method `list_invitations` in `src/mcp/server.rs`, with no input. For each `client.invited_rooms()` entry:
  - Read `name()`, then `cached_display_name()`.
  - Read `canonical_alias()`, `topic()`, and `encryption_state()`.
  - Read `invite_details().inviter_id`, with null on error.
  - Join the result with the registry snapshot.
  - A per-room read failure sets that field to null and does not fail the call.
  - The stub client gives an empty list.
  - Return one text content item with the JSON.
  - The tool never joins, declines, or retries. Do not change `join_room` (FR-013).

**Checkpoint**: `list_invitations` works with `BOT_ENABLED` unset and with it set.

## Phase 5: User Story 3 - Runtime join with bounded retry (Priority: P2)

**Goal**: Temporary failures retry at most 5 times, permanent failures stop at once, and no room has two joins in progress (FR-006 to FR-008, SC-004, SC-005).

**Independent Test**: A mocked join returns 502 and then 200. Exactly two join requests are made and one greeting is sent. A mocked 403 gives one request and `lastError: "permanent 403 M_FORBIDDEN"`.

### Tests for User Story 3

- [ ] T019 [P] [US3] Add unit tests in `src/bot/invites.rs`:
  - `classify()` gives `Permanent` for 403, 404, `M_FORBIDDEN`, `M_NOT_FOUND`, `M_UNSUPPORTED_ROOM_VERSION`, and any other 4xx except 429.
  - `classify()` gives `Temporary` for 429, `M_LIMIT_EXCEEDED`, 5xx, and errors with no client API body.
  - With the default policy, `backoff(n)` is within 20 percent of 30 s, 60 s, 120 s, and 240 s for n = 1 to 4. It never exceeds 18 minutes for large n, which is 15 minutes times 1.2.
  - No attempt is due after 5 attempts or after a `Permanent` error.
- [ ] T020 [P] [US3] Extend `tests/bot_invites.rs` with a test `RetryPolicy { base: 10ms, cap: 50ms, max_attempts: 5 }`:
  - A 502 then a 200 give 2 join requests and 1 greeting.
  - Five 502 responses give exactly 5 join requests.
  - A 403 gives 1 request.
  - A join answer with a 2-second delay, while 20 notifications are sent, never has 2 join requests in progress for the room (SC-005).
  - An invite that is gone from the next `/sync` is pruned from the registry.

### Implementation for User Story 3

- [ ] T021 [US3] Implement `classify(&matrix_sdk::Error) -> JoinError` in `src/bot/invites.rs`, using `Error::Http` with `HttpError::as_client_api_error()` for the status, and `client_api_error_kind()` for the code (`research.md` R5).
- [ ] T022 [US3] Add `RetryPolicy` in `src/bot/invites.rs`, with the default `base 30s, cap 15min, max_attempts 5`. `backoff(n) = min(base * 2^(n-1), cap) * jitter`, with `jitter` in `[0.8, 1.2]`. The jitter comes from `SystemTime::now()` subsecond nanos mixed with a hash of the room id. Do not add `rand`.
- [ ] T023 [US3] Extend `reconcile_pass` in `src/bot/invites.rs`:
  - Skip a room whose entry is `in_flight`, is not yet due, has `attempts >= max_attempts`, or has a `Permanent` `last_error`.
  - Set `in_flight` before the join and clear it after.
  - Raise `attempts`, and store `last_error` and `next_attempt_at` from `backoff`.
  - Log each attempt with the room id, the attempt number, and the error class.
  - Treat `Ok` from `room.join()` as success, including "already a member" (FR-008).
- [ ] T024 [US3] Change the `spawn_reconciler` loop to `tokio::select!` over `notify.notified()` and `sleep_until(earliest next_attempt_at)`, so a retry runs on time while the sync long-poll is idle (`research.md` R2). Take `RetryPolicy` as a parameter, and use the default in `src/main.rs`.

**Checkpoint**: All US1, US2, and US3 tests pass in both feature builds.

## Phase 6: Polish and Release

- [ ] T025 [P] Update `src/AGENTS.md`: add `src/bot/invites.rs`, the new startup order (attach, reconciler, then background sync), `list_invitations`, and the `21 + 2` tool count. Update the root `AGENTS.md` and `.cursor/rules/project-context.mdc` to `21 + 2`.
- [ ] T026 [P] Update `README.md` with `list_invitations` and the automatic-join behavior. Bump `version` in `Cargo.toml` to `0.4.0`, and refresh `Cargo.lock`.
- [ ] T027 Run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features e2ee -- -D warnings`, `cargo test`, and `cargo test --features e2ee`.
- [ ] T028 Run the `@agent0-2` canary in `quickstart.md` only after the user approves the deploy. Record the result in `spec/T0NN-Agent0-2-Invite-Result-YYYYMMDD.md`. Do not upgrade any other agent, tag, or push until the user asks.

## Dependencies

- Phase 1, then Phase 2, then the story phases.
- US1 (Phase 3) depends on T004, T005, and T007.
- US2 (Phase 4) depends on T004 and T006 only. It can run in parallel with US1. Its `trusted` and `attempts` fields show real values once US1 is done.
- US3 (Phase 5) depends on US1 (T011 and T013).
- Phase 6 depends on all stories.

## Parallel Opportunities

- T002 and T003 in Phase 1.
- T006 with T005 in Phase 2.
- In US1: T008 and T009.
- In US2: T014, T015, and T016. The whole US2 phase can run beside US1 after Phase 2.
- In US3: T019 and T020.
- T025 and T026 in Phase 6.

## Implementation Strategy

1. **MVP**: Phases 1 to 3 (US1). This fixes the `@agent0-2` failure: offline trusted invites join at startup.
2. **Second increment**: Phase 4 (US2). Untrusted and failed invites become visible, and an agent can join them with `join_room`.
3. **Third increment**: Phase 5 (US3). Retry and the duplicate-join guard.
4. **Release**: Phase 6, then the canary on `@agent0-2`, then the tag `v0.4.0` when the user asks.
