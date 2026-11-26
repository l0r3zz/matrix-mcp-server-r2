# Invite join 403 race

Date: 2026-11-26

Feature: `.specify/specs/004-invite-reconciliation/`
Task: T029 in `.specify/specs/004-invite-reconciliation/tasks.md`
Code: `src/bot/invites.rs` (`classify`, `join_is_due`, `reconcile_pass`)

This note is the input for T029. T029 was implemented on 2026-11-26. The `@agent0-2` canary passed the same day. The encrypted invite hit this 403, the retry joined, and one greeting was posted. The record is `spec/T028-Agent0-2-Invite-Result-20261126.md`.

## Decision

A 403 is temporary only when its message says the user is not invited, and only while that room stays in the invited set. The message is not stored or logged. `classify()` records `temporary 403 M_FORBIDDEN`, and the existing 5-attempt schedule retries the join. One success posts the greeting once.

These stay one attempt:

- A ban, or any other 403 whose message does not say the user is not invited.
- HTTP 404 and `M_NOT_FOUND`.
- An unsupported room version.
- A 403 after the invite is gone. `prune` drops the room, so the scheduled retry does not send another join.

## What the lab showed

On 2026-11-26, `@admin:v-site.net` invited `@agent0-5:agent0-5-mhs.cybertribe.com` to **Research Requests** (`!tA6-a-sw1In0yUieNqmz0QwzXpWC7VGfvM-sP74wP5Q`, room version 12, `m.megolm.v1.aes-sha2`). `run_mcp` was already running. `BOT_ENABLED` was true. The inviter is trusted.

Synapse times on `v-site.net`:

| Time (UTC) | Event |
|---|---|
| 08:04:43.962 | Federation invite `PUT` to `agent0-5-mhs.cybertribe.com` completed with 200. |
| 08:04:44.284 | `make_join` for `@agent0-5:agent0-5-mhs.cybertribe.com` returned 403. The error text was `You are not invited to this room.` |
| 08:04:44.777 | The Element invite `POST` completed with 200. It had been in progress while the 403 was returned. |

The bot log at 08:04:44 was `decision="join"` for `@admin:v-site.net`, then `invite join failed` with `attempt=1` and `error_class=permanent`. Membership on Synapse stayed `invite`. No greeting was posted. A later `supervisorctl restart run_mcp` cleared the in-memory permanent flag. The invite was still pending, the next join succeeded, and membership became `join`.

The same shape failed the `@agent0-2` canary on 2026-11-11. Both the runtime invite and the encrypted invite returned 200 for the invite, then 403 on `make_join`, with the same error text, one attempt, and `error_class=permanent`. That run is `spec/T028-Agent0-2-Invite-Result-20261111.md`. The status note is item 1 in `spec/Feature-004-Spec-and-Implementation-20261125.md`.

## Why the agent stayed invited

`classify()` in `src/bot/invites.rs` marks HTTP 403, HTTP 404, `M_FORBIDDEN`, `M_NOT_FOUND`, and `M_UNSUPPORTED_ROOM_VERSION` as permanent. Any other 4xx except 429 is permanent too. `join_is_due` returns false when the last error is permanent. The room stays in `list_invitations` with `permanent 403 M_FORBIDDEN`.

That matches the 2026-11-09 clarification and FR-007a. A 403 that will not change stops after one attempt. This 403 did change: the invite was still being saved, and the same invite joined after a process restart. Retry state is in memory, so a restart is what starts the attempts again from zero.

## What T029 has to decide

Research which 403 answers are still final, and which must be tried again while the room remains a pending invite from a trusted sender. A ban, a missing room, an unsupported room version, and an invite that is already gone stay final. A 403 of `You are not invited to this room` in the moment the invite is saved must not leave a still-pending trusted invite stuck for the life of the process.

The fix joins that invite without a process restart and posts the 003 greeting once. `list_invitations` keeps showing the last error until the join succeeds. A second join for the same room does not run while one join is in progress.

Add a test that returns one 403 while the invite remains, then one successful join. A 404, and a 403 after the invite is gone, still stop after one attempt.

Do not add a Cargo dependency. Do not bump `Cargo.toml`. Do not deploy, tag, or push as part of this task. The T028 re-run was approved and passed on 2026-11-26. No tag or push was made.
