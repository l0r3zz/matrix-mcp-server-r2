# Data Model: Invite Reconciliation and Discovery

**Feature**: `004-invite-reconciliation` | **Date**: 2026-11-09

All state is in memory. A restart starts with an empty registry (spec Assumptions).

## InviteRegistry

Shared by the reconciler (`src/bot/invites.rs`) and `list_invitations` (`src/mcp/server.rs`).

```rust
/// Per-room automatic join state. Empty when BOT_ENABLED is false.
pub struct InviteRegistry {
    inner: tokio::sync::Mutex<HashMap<OwnedRoomId, InviteEntry>>,
    /// False until the reconciler has started. list_invitations reports
    /// trust as "unknown" while this is false.
    active: AtomicBool,
}
```

- Created once in `main.rs` as `Arc<InviteRegistry>`, whether the bot is enabled or not.
- The lock is held only to read or update entries, never across `Room::join()`, `invite_details()`, or am-ctl calls.

## InviteEntry

| Field | Type | Meaning |
|-------|------|---------|
| `inviter` | `Option<OwnedUserId>` | From `invite_details().inviter_id`. `None` means missing or malformed. |
| `last_decision` | `Option<Decision>` | Used for decision-change logging (FR-014) |
| `attempts` | `u8` | Join attempts made, 0 to 5 |
| `last_error` | `Option<JoinError>` | The last failed attempt |
| `next_attempt_at` | `Option<tokio::time::Instant>` | `None` means it may run now, or never if exhausted or permanent |
| `in_flight` | `bool` | True while this room's join request runs (FR-006) |

## Decision

```rust
pub enum Decision {
    /// The inviter is in the trusted set.
    Join,
    /// The inviter is not in the trusted set.
    SkipUntrusted,
    /// The inviter is missing or malformed.
    SkipNoInviter,
}
```

`decide(inviter: Option<&UserId>, trust: &TrustMemory) -> Decision` is pure and is compared by exact full user id (FR-004). An empty trusted set gives `SkipUntrusted` (fail-closed, FR-003).

## JoinError

```rust
pub struct JoinError {
    pub class: ErrorClass,   // Permanent | Temporary
    pub status: Option<u16>, // HTTP status when known
    pub code: Option<String>, // Matrix errcode, for example "M_FORBIDDEN"
}
```

- The `Display` output is `"<class> <status> <code>"`, for example `permanent 403 M_FORBIDDEN`.
- It never holds the server error message body, which may echo request data.

## State transitions per room

```text
(absent) --invite seen--> Pending(attempts=0)
Pending --decide=Skip*--> Pending            (no attempt; log only on change)
Pending --decide=Join, due, not in_flight--> InFlight
InFlight --Ok--> (removed) + greeting once
InFlight --Temporary, attempts<5--> Pending(next_attempt_at = now + backoff)
InFlight --Temporary, attempts=5--> Exhausted (stays listed with last_error)
InFlight --Permanent--> Failed (stays listed with last_error)
any --room no longer invited--> (removed)    (FR-016)
```

- `Exhausted` and `Failed` are `Pending` entries whose `next_attempt_at` is `None` and whose `attempts > 0`, with a terminal `last_error`.
- A later trust change does not restart them.
- A process restart clears them, so the next start retries once from zero.

## Retry schedule

`backoff(n) = min(30s * 2^(n-1), 15min) * jitter`, where `jitter` is in `[0.8, 1.2]` and `n` is the failed attempt number. There are at most 5 attempts (FR-007).

## Reconcile pass (pseudo)

```text
rooms = client.invited_rooms()
registry.prune(not in rooms)
if rooms empty: return                       # no am-ctl call (FR-003)
trust.refresh() once
for room in rooms:
    inviter = room.invite_details().inviter_id (or None)
    d = decide(inviter, trust)
    log if d != entry.last_decision
    if d == Join and due and attempts < 5 and not terminal:
        mark in_flight; drop lock
        result = room.join()
        update entry; on Ok: remove entry, send greeting once
```
