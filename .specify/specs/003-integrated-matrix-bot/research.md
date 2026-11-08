# Research: Integrated Reactive Matrix Bot

**Date**: 2026-11-05
**Spec**: `.specify/specs/003-integrated-matrix-bot/spec.md`

Every item below is resolved. Nothing remains as NEEDS CLARIFICATION.

## 1. Which single device survives

**Decision**: One Matrix device per Agent-Matrix agent. The surviving device is the one already recorded in `CRYPTO_STORE_PATH/.device-id` and the SQLite store opened by `src/matrix/client.rs`. Retire the AgentZeroBot device. Do not call `--logout`. Do not reset the password. Do not upload a second set of device keys.

**Rationale**: The operator required one device per agent. Feature 002 already cross-signed this store and refuses a second opener with `KeyStoreUnavailableError`. The live bot device is a separate token in `matrix-bot/.env` and cannot share that store.

**Alternatives considered**: Keep AgentZeroBot and drop the MCP device. That throws away the cross-signed store. Mint a fresh device for the unified process. That is a second device during cutover and needs a logout or password reset, which this feature forbids.

## 2. Who posts the Matrix reply

**Decision**: `src/bot/` posts the only reply with `room.send()` on the shared client. The text sent to Agent Zero keeps the live preface that tells Agent Zero to answer in plain conversational text and not to send messages through Matrix MCP tools.

**Rationale**: Success criteria SC-001 and SC-002 say the reply comes from `matrix-mcp-server-r2`. If Agent Zero also calls `send-message`, the room gets two replies.

**Alternatives considered**: Let Agent Zero post through MCP and make the bot a silent forwarder. That breaks the fast commands, which must not call Agent Zero, and it races the bot reply.

## 3. When the dispatcher turns on

**Decision**: `BOT_ENABLED` defaults to false. While it is false, `start_background_sync` stays the 002 loop and no invite or message handler is registered. Set it true only after `supervisord` program `run_bot` is stopped.

**Rationale**: `matrix-bot-rust` is a separate `reqwest` process. It can still be running when this binary starts, and it will answer the same trigger. The crypto-store lock does not stop it, because that bot does not open the store.

**Alternatives considered**: Default the flag true so a new container is live on first boot. The current fleet still runs `run_bot`, so the first boot would double-reply.

## 4. Trigger grammar and fast commands

**Decision**: Match the live bot, case-insensitive, after Matrix mention pills are stripped. A prefix is required in every room, including a two-member room. Prefixes are `{agent_short}:`, `{agent_short},`, `@{agent_short}`, `@all-agents:`, `@all-agents,`, plus `{display_name}:`, `{display_name},`, and `@{display_name}` when `BOT_DISPLAY_NAME` is set and differs from the short name. Only `m.text` is considered. After the prefix, the remaining text is compared as a whole, trimmed and lowercased:

- `ping` replies `PONG`
- `hello` replies `Hello! I'm {identity}, online and ready.`
- `whoami` replies `I'm {identity} ({user_id}).`

`identity` comes from `AGENT_IDENTITY`, falling back to `BOT_DISPLAY_NAME`. Anything else is sent to Agent Zero. `@all-agents` is only a prefix on this agent's own sync. Each enabled agent in the room answers for itself. There is no extra fan-out call.

**Rationale**: SC-001 names `ping` and `hello` in both an encrypted room and Fleet HQ Clean. The live prefixes are a display-name prefix, `@agent0-N:`, and `@all-agents:`. The process builds `@agent0-N` from its own user id.

**Alternatives considered**: Answer any message in a two-member room. That wakes the agent on traffic that was not addressed to it and can loop with another agent.

## 5. Sync, dedup, and Agent Zero latency

**Decision**: Register SDK event handlers on the existing `Client`. Keep one `sync_once` loop in `start_background_sync`. Ignore `sync_cursor.json`. Persist at most 1000 processed event ids in `processed_event_ids.json`. Drop an event whose `origin_server_ts` is older than 300000 ms. Ignore `sender == client.user_id()`. Spawn the Agent Zero HTTP call on its own task. Set the typing notice before that call and clear it when the task finishes.

**Rationale**: The SDK already advances `next_batch` inside the sync response (`client.rs` stores `response.next_batch` on the next `SyncSettings`). A second cursor double-counts. Waiting on `/api/api_message` inside the sync task delays to-device events that carry room keys.

**Alternatives considered**: Keep the bot's raw `/sync` loop beside `sync_once`. That is a second sync on the same token and device, which this feature exists to remove.

## 6. Unable to decrypt

**Decision**: If the SDK cannot decrypt an event, emit a structured warning that includes the event id and the reason, and send no room message. Do not include ciphertext or plaintext in the log. MCP read tools keep the 002 marker `[Encrypted message - unable to decrypt ({reason})]`.

**Rationale**: Principle VI forbids a silent drop and forbids a panic. The dispatcher cannot see a trigger in an event it cannot decrypt, so a room reply would be a guess. The marker operators already see is on the read path.

**Alternatives considered**: Post the UTD marker into the room from the bot. That announces every undecryptable event to every member.

## 7. Trusted invites

**Decision**: On an invite, `GET {AMCTL_URL}/api/v1/trusted-senders` with header `X-API-Key` from `AMCTL_API_KEY`. Timeout 5 seconds. A successful body replaces the in-memory set. If the call fails or the set is empty, use `TRUSTED_SENDERS` (comma-separated Matrix ids). If that is also empty, reject the invite and log the room id and sender. Join only when the inviter is in the set. After a successful join, send `Hello! I'm {display_name}, an AI agent powered by Agent Zero.` There is no compiled-in user id and no compiled-in API key.

**Rationale**: SC-003 accepts `@admin:v-site.net` and ignores everyone else. Those ids belong in am-ctl or in the env fallback, not in the binary. The live bot's compiled fallback and default API key are not part of this design.

**Alternatives considered**: Fail open and join every invite when am-ctl is down. That lets an untrusted sender pull the agent into a room.

## 8. Agent Zero call and the reply body

**Decision**: Read the credential from `/a0/usr/workdir/matrix-bot/.a0-api-key`. On HTTP 401, re-read the file once and retry once. `POST` JSON `message` and `context_id` to `A0_API_URL` (default `http://localhost:80/api/api_message`). Store a returned context id back into `room_contexts.json` for that `room_id`. Render the reply with `pulldown-cmark` and send it with `room.send()`. Split a reply into chunks of at most 4000 characters.

**Rationale**: The spec requires the key reload, the context file, the HTML render, and the chunk size. Keeping `room_contexts.json` at its current path preserves SC-002 continuity across the cutover.

**Alternatives considered**: Move `room_contexts.json` into the crypto store. That mixes conversation ids with Olm state and breaks the file the running bot already updates.

## 9. What 002 must still do

**Decision**: Leave these as they are: `history_visibility=invited`, cross-signed recipient strategy, no TOFU, exclusive `.store.lock`, the 20+2 tool names, and `EncryptedRoomError` staying absent.

**Rationale**: 003 sits on the 002 client. Changing those contracts is a different feature.

**Alternatives considered**: None. The constitution locks them.
