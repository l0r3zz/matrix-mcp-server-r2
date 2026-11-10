# Implementation Plan: Invite Reconciliation and Discovery

**Branch**: `004-invite-reconciliation` | **Date**: 2026-11-09 | **Spec**: `.specify/specs/004-invite-reconciliation/spec.md`

**Input**: Feature specification from `.specify/specs/004-invite-reconciliation/spec.md`

## Summary

`v0.3.1` joins a trusted invite only from the `StrippedRoomMemberEvent` handler. `create_matrix_client` runs the first `sync_once` before `bot::attach` registers that handler (`src/main.rs`). An invite delivered in the first sync reaches no handler, and no code reads `client.invited_rooms()`.

004 replaces the event-driven join with one reconciler task in `src/bot/invites.rs`. The task runs one pass after `bot::attach`, and one pass after each successful background sync. Each pass reads `client.invited_rooms()`, decides each invite against the 003 trusted set, and joins trusted rooms one at a time with a bounded retry schedule. A shared `InviteRegistry` holds per-room attempt state. The new MCP tool `list_invitations` reads `client.invited_rooms()` and that registry. The tool surface becomes 21 + 2.

## Technical Context

**Language/Version**: Rust 1.99, edition 2021 (constitution pin)

**Primary Dependencies**: Existing only. `matrix-sdk` 0.19.1 (`Client::invited_rooms`, `Room::invite_details`, `Room::join`, `Room::encryption_state`, `HttpError::as_client_api_error`), `ruma` 0.17, `rmcp` 1.2, `tokio` 1.43 (`sync::Notify`, `sync::Mutex`). No new crate. Jitter comes from the system clock, so `rand` is not added.

**Storage**: In-memory `InviteRegistry` only. No new file. `processed_event_ids.json` is no longer written for invites. Existing `invite:` entries in it stay and are ignored.

**Testing**: `cargo test` and `cargo test --features e2ee`. Unit tests for the decision, the retry schedule, error classes, and decision-change logging in `src/bot/invites.rs`. Wiremock integration tests in `tests/bot_invites.rs`: a `/sync` response with `rooms.invite`, a mocked `POST /_matrix/client/v3/join/{roomId}`, and assertions on the outbound join count. The encrypted-room canary is a lab check in `quickstart.md`.

**Target Platform**: Agent-Matrix container, the same binary at `/a0/usr/workdir/matrix-mcp-server/`.

**Project Type**: The existing Axum process: Streamable HTTP MCP at `/mcp`, `GET /health`, and the in-process `src/bot/` dispatcher.

**Performance Goals**: A trusted invite is joined within 60 seconds after `/health` is healthy, or within 60 seconds of arrival at runtime (SC-001, SC-002). A pass with no pending invites makes no HTTP call.

**Constraints**:
- One Matrix device and one crypto store (Principle IV).
- Cross-signed trust is unchanged.
- No new Cargo dependency (src `AGENTS.md`: ask first).
- No secret in logs or tool output.
- A pass never runs on the sync task.
- At most one pass runs at a time.
- At most one join per room is in progress.

**Scale/Scope**: One reconciler per agent, five lab agents, and normally zero to a few pending invites. `list_invitations` returns at most 100.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (Unified Tool Surface)**: One new tool, `list_invitations`. No existing tool name, schema, or behavior changes, and `join_room` is unchanged. The surface moves from 20 + 2 to 21 + 2. **Amendment required**: constitution v2.1.0 (MINOR). The amendment also updates `spec/matrix-mcp-v1-v2.yml`.
- **Principle II (Feature-Gated E2EE)**: The reconciler and the tool compile in both builds. `Room::encryption_state` exists without the `e2ee` feature, so no feature gate is needed.
- **Principle III (Rust Idioms)**: No `unwrap` or `expect` in production paths. The registry lock is a `tokio::sync::Mutex` and is never held across a join request. New public items have doc comments.
- **Principle IV (Single Store Owner)**: The reconciler uses the shared `Client`. It opens no store and starts no second sync.
- **Principle V (Security First)**: Logs carry the room id, inviter, decision, attempt number, and error class. The tool output has room metadata only. `AMCTL_API_KEY` stays unlogged. The trust rule stays fail-closed.
- **Principle VI (Transparent Encryption)**: Joining an encrypted room uses the normal SDK join. The crypto store, device trust, and history visibility are not touched.
- **Principle VII (Test Everything)**: New unit tests and wiremock integration tests. CI runs both feature builds.
- **Principle VIII (Container-Aware)**: `/health` is unchanged. Retry state is in memory and restarts from zero.
- **Principle IX (Incremental Delivery)**: **Amendment required**: add `004-invite-reconciliation` to the roadmap list. 004 is off unless `BOT_ENABLED=true`. `list_invitations` works either way.
- **Principle X (Spec-Driven)**: This plan, `research.md`, `data-model.md`, `contracts/list-invitations.md`, and `quickstart.md` are the design.
- **Naming (Development Workflow)**: Constitution v2.0.4 names MCP tools in snake_case. The registered names already match, because rmcp uses the method name (for example `join_room`). `spec/matrix-mcp-v1-v2.yml`, `README.md`, and `.cursor/rules/` still use kebab-case. **Amendment required**: rename every tool name in those files to the registered snake_case name (T002, T025, T026). The runtime names do not change, so Principle I stability holds. `list_invitations` follows the real convention.

Gate result: PASS. T001 bumps the constitution to v2.1.0 and adds 004 to the roadmap. T002, T025, and T026 rename the remaining kebab-case tool names.

Post-design re-check: PASS. The contract adds one read-only tool. The data model adds in-memory state only. No locked stack version changes.

## Project Structure

### Documentation (this feature)

```text
.specify/specs/004-invite-reconciliation/
├── spec.md
├── plan.md                    # this file
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── list-invitations.md
├── checklists/
│   └── requirements.md
└── tasks.md                   # created by /speckit-tasks
```

### Source Code

```text
src/bot/invites.rs           # new: InviteRegistry, decide(), retry schedule, error class, reconcile pass, reconciler task
src/bot/mod.rs               # attach(): drop the join path of register_invite_handler; return the Runtime for the reconciler
src/bot/trusted.rs           # unchanged API; greeting() reused
src/matrix/client.rs         # start_background_sync(client, after_sync: Option<Arc<Notify>>): notify after each Ok
src/mcp/server.rs            # list_invitations tool; MatrixMcpServer::with_invites(registry)
src/main.rs                  # create the registry and Notify; spawn the reconciler; abort it on shutdown
tests/bot_invites.rs         # new: wiremock /sync rooms.invite + /join
tests/server_tool_list_tests.rs  # tool list includes list_invitations
spec/matrix-mcp-v1-v2.yml    # list_invitations contract
.specify/memory/constitution.md  # v2.0.4 now; T001 bumps it to v2.1.0
src/AGENTS.md                # DOX: invites.rs, list_invitations, startup order, snake_case tool names
```

**Structure Decision**: One crate. The reconciler is a new module in `src/bot/`, beside `trusted.rs`. The registry type lives there and is shared with `src/mcp/server.rs` through `Arc`.

## Complexity Tracking

| Decision | Why Needed | Simpler Alternative Rejected Because |
|----------|------------|-------------------------------------|
| Remove the join from the 003 invite event handler | One join path makes FR-006 hold with no cross-path lock | Keeping both paths needs a shared in-flight guard, and two paths log twice |
| A `Notify` from the sync loop, not a timer | FR-002 says "after each successful sync". `Notify` coalesces, so a slow pass never queues a backlog | A fixed timer misses the order guarantee, and per-sync spawning can overlap passes |
| Sequential joins inside one pass | A pass seldom holds more than a few invites, and FR-006 then needs no per-room lock | Concurrent joins need a join set and more shutdown handling for no gain |
| Clock-based jitter | No new dependency | `rand` needs an "ask first" approval for a few lines of jitter |
| `with_invites()` builder on `MatrixMcpServer` | The ten existing `MatrixMcpServer::new(client, config)` calls keep compiling | Changing `new` touches every e2ee test |
