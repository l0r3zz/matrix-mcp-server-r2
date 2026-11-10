# Invite Reconciliation -- Spec Kit Approach

Date: 2026-11-09
Input: `20261109-222129-matrix-mcp-trusted-invitation-acceptance-60-final.md` (Galadriel, `@agent0-2`)
Base: `main` at `d1bde46` (tag `v0.3.1`)

## Summary

The input task asks for invite discovery (`list_invitations`) and trusted auto-join at startup and at runtime. `v0.3.1` already has a trusted-invite handler in `src/bot/mod.rs`. It misses invites that arrive while the service is down, and nothing lists pending invites. Build this as a new Spec Kit feature, `004-invite-reconciliation`, on top of `main`.

## Provenance (input section 4)

- The authoritative source is this repo, `main` at `d1bde46`, tag `v0.3.1`. It uses `matrix-sdk` 0.19.
- All five agents run image `ghcr.io/l0r3zz/matrix-mcp-server-r2:0.3.1`. Each `/health` reports version `0.3.1` with `botEnabled` true.
- `/opt/agent-zero/matrix-mcp-server-r2` on g2s is a stale `matrix-sdk` 0.10 checkout. Do not build it. Do not use it as a base.
- The `002-e2ee-cross-signing` branch is merged history. It is not the base for new work.

## What v0.3.1 already does

- `register_invite_handler` (`src/bot/mod.rs`) handles `StrippedRoomMemberEvent` for the agent's own user id with membership `invite`.
- The inviter is checked against am-ctl `GET /api/v1/trusted-senders`. `TRUSTED_SENDERS` is the fallback only while the set is empty. A failed lookup with an empty fallback does not join.
- A trusted invite joins and sends one greeting. An untrusted invite logs `reject_untrusted`.
- A dedup key `invite:{room_id}:{sender}` is kept in the bot state file.

## Gaps against the input task

1. **Invites that arrive while the service is down.** `initialize_client` runs the first `sync_once` (`src/matrix/client.rs:130`) before `bot::attach` registers handlers (`src/main.rs:100`). An invite delivered in that first sync reaches no handler. Later syncs do not deliver it again. This matches the Galadriel case: the agent0-2 logs have no `reject_untrusted` line and no join attempt for `!tA6-a-sw1In0yUieNqmz0QwzXpWC7VGfvM-sP74wP5Q`.
2. **No reconciliation of current state.** No code calls `client.invited_rooms()`.
3. **No discovery tool.** No MCP tool lists pending invites. An agent can join only when it already knows the room id.
4. **No retry.** A failed `room.join()` logs a warning and returns. Nothing tries again.
5. **No in-flight guard.** Two passes could join the same room at the same time.

## Decisions to settle in `/speckit-clarify`

| # | Question | Input task says | v0.3.1 / 003 says |
|---|----------|-----------------|-------------------|
| 1 | Which allowlist governs auto-join? | A new `MATRIX_AUTO_JOIN_INVITERS`. Do not reuse the message trusted-sender list. | am-ctl trusted senders, then `TRUSTED_SENDERS` (003 clarification). |
| 2 | Is auto-join on or off by default? | Off when unset. | On when am-ctl returns a non-empty set. |
| 3 | Is the greeting still sent? | Not specified. | Yes, once per trusted join. |
| 4 | Tool name | `list_invitations` | Existing tools use underscore names, for example `send_message`. |
| 5 | Canary agent | `@agent0-2` | The repo rule names `@agent0-5` as the lab target. |
| 6 | Retry ceiling and backoff | Bounded, with jitter. Values not set. | None today. |
| 7 | Reconcile trigger | After the first sync and after each sync, or from an SDK handler. | Event handler only. |

Questions 1 and 2 change 003 behavior. If the answer is a separate allowlist, record it as a 004 change to the 003 contract, and amend the constitution.

### Decisions (operator, 2026-11-09)

1. **Allowlist:** keep the 003 source. am-ctl trusted senders, then `TRUSTED_SENDERS` while the set is empty. No new variable. A failed lookup with an empty fallback does not join.
2. **Default:** unchanged from 003. Auto-join applies when the trusted set is non-empty.
3. **Greeting:** yes, once per room. This covers rooms joined at startup and at runtime.
4. **Tool name:** `list_invitations`.
5. **Canary:** `@agent0-2`, where the bug was found. This is a stated exception to the `@agent0-5` default. Specs still use `@agent0-N`.
6. **Retry:** up to 5 attempts per room. Exponential backoff starts at 30 seconds, is capped at 15 minutes, and has jitter. After the fifth failure, stop and show the last error in `list_invitations`.
7. **Trigger:** reconcile once after `bot::attach`, and again after each background sync.

Decisions 1 and 2 keep the 003 trust contract, so the constitution changes only for the tool count.

## Constitution impact

- The constitution fixes the tool surface at `20 + 2`. `list_invitations` makes it `21 + 2`. Amend `.specify/memory/constitution.md` with a minor version bump, and update `spec/matrix-mcp-v1-v2.yml`.
- Principle text for the reactive bot names "trusted-invite auto-join". Update it if question 1 changes the trust source.

## Recommended Spec Kit sequence

1. `/speckit-specify`: create `004-invite-reconciliation` from the input task. Drop input section 4, because provenance is settled above. Point at this report.
2. `/speckit-clarify`: settle the seven decisions above.
3. `/speckit-constitution`: amend the tool count and the trust source, if needed.
4. `/speckit-plan`: one reconciler module in `src/bot/`. It runs once after `bot::attach`, and again after each background sync. It also covers the manual `list_invitations` read path, per-room in-flight state, and bounded retry.
5. `/speckit-tasks`, then `/speckit-analyze`.
6. `/speckit-implement`. Run `cargo fmt --all --check`, then clippy and tests for the default build and `--features e2ee`.
7. `/speckit-converge` before the canary.
8. Canary on one agent. Run the offline invite test, the runtime invite test, and the untrusted negative test from input section 7. Then upgrade the other agents one at a time.
9. Release `v0.4.0`. A new MCP tool and a startup behavior change are a minor release. Then bump the target tag in agent-matrix.

## Out of scope

- Continuwuity stale-membership and federation behavior.
- Decrypting encrypted history sent before the join.
- Any change to the cross-signed trust policy or the crypto store.
