# Research: Invite Reconciliation and Discovery

**Feature**: `004-invite-reconciliation` | **Date**: 2026-11-09

## R1. Why the offline invite is missed

**Decision**: The first pass runs after `bot::attach`, from `client.invited_rooms()`, not from events.

**Rationale**: `src/main.rs` calls `matrix::create_matrix_client`, which runs the first `sync_once` (`src/matrix/client.rs`). Only after that does `bot::attach` register `register_invite_handler`, and then `start_background_sync` starts. The first sync stores the invite in room state, but no handler sees its event. Later syncs carry no new invite event for that room. `client.invited_rooms()` reads the stored state, so it sees the invite in every case.

**Alternatives considered**: Register handlers before the first sync. That moves bot code into `initialize_client` and couples the E2EE bootstrap order to the bot. It also leaves invites that were skipped earlier, for example while the inviter was untrusted, with no second chance.

## R2. Trigger after each background sync

**Decision**: `start_background_sync(client, after_sync: Option<Arc<tokio::sync::Notify>>)` calls `notify_one()` after each `Ok` from `sync_once`. The reconciler task loops on `tokio::select!` over `notify.notified()` and `sleep_until(next_retry_due)`.

**Rationale**:
- `Notify` holds at most one permit, so a slow pass sees one wakeup, not a queue.
- The pass runs on the reconciler task, so the sync loop never waits on a join.
- One task means at most one pass at a time.
- The retry deadline wakes the task even when the long-poll sync is idle, so the 30-second first retry is on time.

**Alternatives considered**:
- `client.add_event_handler` on a sync-response type. 0.19 has no stable "sync finished" handler with the same ordering.
- A timer loop. It ignores FR-002 ("after each successful sync") and wakes when nothing changed.

## R3. Reading invite data

**Decision**: Use the matrix-sdk 0.19.1 room API:
- `Client::invited_rooms()`
- `Room::invite_details().await?.inviter_id`
- `Room::name()` and `cached_display_name()`
- `Room::canonical_alias()`
- `Room::topic()`
- `Room::encryption_state()`

**Rationale**:
- `invite_details` returns `inviter_id: OwnedUserId` from our own stripped `m.room.member` event. It fails with `WrongRoomState` or `EventMissing` when the room is not in the invited state or the event is gone. An error means the inviter is missing (FR-005): never join, and list it with `inviter: null`.
- All of these are available without the `e2ee` feature.
- `encryption_state()` reads stripped state with no network call, and it may be `Unknown`.

## R4. The 003 event handler

**Decision**: Remove the join path from `register_invite_handler`. The reconciler owns every automatic join.

**Rationale**:
- After each sync, the reconciler sees the same invite that the handler would see, within one pass.
- Two join paths would need a shared per-room guard for FR-006 and would log twice (FR-014).
- The handler's `invite:{room}:{sender}` dedup entry in `processed_event_ids.json` is no longer written. Old entries stay harmless.

**Alternatives considered**: Keep the handler as a fast path that calls `notify_one()`. The background sync already notifies after the same response, so the extra path adds nothing.

## R5. Error classes

**Decision**: Classify `matrix_sdk::Error` from `Room::join()` as follows:

- **Permanent**:
  - `Error::Http(e)` where `e.as_client_api_error()` has status 403 or 404.
  - Error kind `Forbidden`, `NotFound`, or `UnsupportedRoomVersion`.
  - Any other 4xx except 429.
- **Temporary**:
  - Status 429 or kind `LimitExceeded`.
  - Any 5xx.
  - Transport and timeout errors (no client API error body).
  - Any other `matrix_sdk::Error`.
- **Already joined**: `Room::join()` returns `Ok` when the server reports the user is already a member. It is success (FR-008).

**Rationale**: Other 4xx codes mean the request will not succeed without a change, so retries waste attempts. Unknown transport errors are usually transient.

## R6. Retry schedule and jitter

**Decision**: After failed attempt `n` (1-based), the delay is `min(30 s * 2^(n-1), 15 min)`, scaled by a factor in `[0.8, 1.2]`. The run stops after attempt 5. The factor comes from `SystemTime::now()` subsecond nanos mixed with a hash of the room id.

**Rationale**:
- The nominal delays are 30 s, 60 s, 120 s, and 240 s. The fifth attempt is the last, so the 15-minute cap is never reached with 5 attempts. The cap is kept to match the clarified rule in case the attempt count changes.
- Clock-plus-room-id jitter spreads agents and rooms without adding `rand`. The spec does not require cryptographic randomness.

## R7. Trust read per pass

**Decision**: If a pass finds one or more invited rooms, it calls `trusted::refresh` once, then decides every invite from that one snapshot. If there are no invited rooms, it makes no am-ctl call.

**Rationale**: FR-003. A pass for an empty invite list at each sync would otherwise call am-ctl about every 30 seconds per agent.

## R8. Decision-change logging

**Decision**: Each registry entry keeps `last_decision`. A pass logs the decision only when it differs from `last_decision`, then stores it. Join attempts and outcomes are always logged.

**Rationale**: FR-014. An untrusted invite is seen on every sync, which would log about 2,800 lines a day per invite without this rule.

## R9. Greeting failure

**Decision**: After a successful join, the reconciler sends `trusted::greeting(display_name)` once. A send failure is logged at `warn` and not retried. The registry entry is removed on join success either way.

**Rationale**: This matches 003. A retried greeting could post twice if the first send reached the server but the response was lost.

## R10. Shutdown

**Decision**: `main.rs` keeps the reconciler `JoinHandle` and aborts it with `sync_handle` and `cleanup_handle` in the graceful shutdown block.

**Rationale**: FR-017. Retry waits are `sleep_until` inside the task, so aborting the task ends them. No join is half-applied: the SDK join is a single request.

## R11. Tool naming

**Decision**: The new tool is named `list_invitations`. Constitution v2.0.4 already uses snake_case for every tool name.

**Rationale**: rmcp derives the tool name from the method name. Every registered tool is snake_case, for example `join_room` and `list_joined_rooms`. The v2.0.3 kebab-case examples did not match the running server.
