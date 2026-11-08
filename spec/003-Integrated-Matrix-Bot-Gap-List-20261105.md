# 003 Integrated Matrix Bot -- Gap List vs the 40-line Spec

**Date**: 2026-11-05
**Repo**: `matrix-mcp-server-r2`
**Branch**: `003-integrated-matrix-bot` at `05cf798` (tracks `origin/003-integrated-matrix-bot`; same commit as `main` today)
**Feature pointer**: `.specify/feature.json` -> `.specify/specs/003-integrated-matrix-bot`
**Spec**: `.specify/specs/003-integrated-matrix-bot/spec.md` (40 lines, Status: Planned). No `plan.md`, `tasks.md`, or `data-model.md` yet.
**Sources**: constitution v2.0.3, 002 spec (Accepted), consolidation analysis 2026-10-29, `src/matrix/client.rs`, agent-matrix `templates/matrix-bot/rust/src/main.rs` (v0.1.2 behavior), OpenBrain captures through 2026-11-05.

No production bot code in this pass. October 2026 notes about a two-tier `-r2` tool API, one shared access token for bot and MCP, and Continuwuity without E2EE are superseded by the 2026-11-05 captures and by 002 as shipped.

## Branch

Work stays on `003-integrated-matrix-bot`. `main` and `002-e2ee-cross-signing` are not the working branch. `002` remains at `04ba258`. This checkout has not diverged from `main` yet.

## What the draft already locks

- Option 2 from the 2026-10-29 consolidation analysis: absorb `matrix-bot-rust` into `src/bot/`, gated by `BOT_ENABLED=true`.
- One process, one `matrix-sdk` 0.19 client, one `.device-id`, one SQLite store, one `/sync` loop.
- Four behaviors to port: trusted-sender auto-join, dedup plus self-suppression plus trigger prefixes, `ping` / `hello` / `whoami`, `room_contexts.json` plus `POST /api/api_message` plus chunked HTML reply.
- Success: `run_bot` stopped; ping and a real question work in Fleet HQ Clean and in an encrypted room.

## Gaps that change acceptance or rollout

1. **Which device survives.** The draft says "one `MATRIX_ACCESS_TOKEN`" and does not name the device. The 2026-11-05 operating rule is the opposite until 003 ships: bot device `AgentZeroBot` and MCP device `crypto-store/.device-id` must stay apart. Sharing one token caused T014 `missing_key` because the first `/sync` consumed to-device room keys. On agent0-1 the `.env` label `AgentZeroBot` is stale; the MCP store already holds the cross-signed device. Unifying onto the bot device would abandon that store. Unifying onto the MCP device retires the bot token and the `run_bot` process. This choice is still open.

2. **Who posts the reply.** `matrix-bot-rust` sends Agent Zero a system preface that forbids Matrix MCP tools, then posts the HTTP body itself (`main.rs` around the `api_message` call). That exists so Agent Zero does not also call `send-message`. The draft never mentions the preface. Dropping it makes every triggered message a double post. Keeping it preserves today's single reply.

3. **Cutover while `run_bot` is still up.** Success criteria assume `run_bot` is already stopped. They do not say `BOT_ENABLED` defaults off, or that enabling it on a live agent before stopping `run_bot` double-replies in Fleet HQ. `am-ctl` still health-checks the bot process and `sync_cursor.json`. Retiring `run_bot` is an agent-matrix supervisord change. `create-instance.sh`, Caddy, Continuwuity templates, and `destroy-and-rebuild` stay out of this feature unless explicitly requested. `sync-fleet.sh` stays retired.

4. **Trigger grammar vs the binary that actually runs.** The draft lists `@agent0-N`, `agent0-N:`, `<display_name>:`, and `@all-agents:`. The Rust bot also matches comma forms (`agent0-1,`, `@all-agents,`), `@<display_name>`, and is case-insensitive. It strips mention pills before the check. It requires a prefix even in a two-member room, on purpose, to stop agent-to-agent loops. Display names in the lab are Gandalf (agent0-1) and Galadriel (agent0-2), not the localpart. The draft does not say whether `Gandalf:` is in the first acceptance set, or whether a bare DM with no prefix is ignored.

5. **Decrypt failure on the reactive path.** Constitution VI says unable-to-decrypt events get an explicit marker on MCP reads. The draft does not say what `src/bot/` does when an encrypted event does not decrypt: stay silent, log without the plaintext, or post the same marker into the room. Posting the marker can spam a room. `start_background_sync` today only loops `sync_once` and stores `next_batch`. It registers no room handler. A handler that awaits Agent Zero inside that loop stalls to-device key delivery for the same client.

6. **State files and the second cursor.** The bot persists `room_contexts.json`, `sync_cursor.json`, and up to 1000 event ids under the bot workdir, and drops events older than 300000 ms. After absorption, `matrix-sdk` owns the sync token inside the client. A second `sync_cursor.json` would fight that token. The draft says "bounded" and does not name the bound, the directory, or that the SDK cursor replaces the bot cursor. It also omits the live 401 path: reload `.a0-api-key` and, on a stale Agent Zero context, drop that room's `context_id` and retry once.

7. **Trusted-sender failure mode.** The draft says query `TRUSTED_SENDERS_URL` and fall back to an env list. The binary, when am-ctl returns nothing, inserts a hardcoded pair (`@admin:v-site.net` and `@l0r3zz:v-site.net`) and will use a baked-in am-ctl API key if `AMCTL_API_KEY` is unset. 003 must not copy that baked-in key. Fail-closed (join nothing) vs fail-open (hardcoded humans) is not chosen. Join retries (4 attempts, greeting after join) and the greeting string are also unspecified. The live greeting is `Hello! I'm {display_name}, an AI agent powered by Agent Zero.` Fast replies that are specified only as names in the draft are, in code: `ping` -> `PONG`; `hello` -> `Hello! I'm {identity}, online and ready.`; `whoami` -> `I'm {identity} ({user_id})`.

8. **Message types and fan-out.** Only `m.text` is handled. Media, edits, redactions, reactions, and threads are absent, so they are easy to treat as accidental scope. `@all-agents:` wakes every joined agent. The draft does not say whether the first lab success includes that prefix, or whether an agent reply is forbidden from containing another agent's trigger.

9. **002 contracts not restated.** These are already shipped and must stay true. The 40-line draft does not say so:
   - `create_room` history visibility stays `invited`. `joined` broke federation join on Continuwuity 26.9.1 (rc.2, `a8d806e`).
   - Cross-signed room keys only. No TOFU.
   - One process may open `CRYPTO_STORE_PATH`. The second open stays `KeyStoreUnavailableError`.
   - No `--logout` password reset. That deletes the MCP device.
   - No `-r2` tool split and no `EncryptedRoomError` refusal.
   - `matrix-bot` v0.1.2 still cannot read `m.room.encrypted`. That remains the reason for 003, not a bug to patch inside the old binary.

10. **Build and lab shape.** The draft does not say the bot runs in a non-`e2ee` build for plaintext rooms, that `SKIP_MATRIX_INIT=true` leaves the dispatcher off, or which agent is the canary. Historical canary was agent0-5. The success lines name agent0-1 (Gandalf). Fleet agents already run r2 `v0.2.0-rc.2` with E2EE on. Proof is ping plus one real question in Fleet HQ Clean and in one encrypted room, with `run_bot` stopped on that agent only.

## Not gaps (do not reopen)

- Unified 20+2 MCP surface, `matrix-sdk` 0.19, Rust 1.99, SQLite store, startup cross-signing. Shipped in 002 (`v0.2.0`, PR #1, `59ca7cb`). Companion agent-matrix `v0.92` (PR #3, `9c33116`) is the device-split tag. Do not merge or retag unless asked.
- Architecture 1 (bot as a second process that calls MCP) was rejected on 2026-10-29 in favor of Option 2.
- `sync-fleet.sh` is retired. Deploys use `am-ctl` or `deploy-mms-r2-to-agent.sh`.

## Clarifications still owed before plan

The highest-impact open choice is which Matrix device the single client keeps. Reply ownership, `BOT_ENABLED` default, 1:1 trigger policy, and unable-to-decrypt behavior are the next decisions if they are not accepted as "match `matrix-bot-rust` except where E2EE forces a change."
