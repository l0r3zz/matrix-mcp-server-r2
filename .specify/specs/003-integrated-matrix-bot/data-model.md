# Data Model: Integrated Reactive Matrix Bot

**Date**: 2026-11-05
**Spec**: `.specify/specs/003-integrated-matrix-bot/spec.md`

The 002 on-disk crypto layout is unchanged. See `.specify/specs/002-e2ee-cross-signing/data-model.md`. This feature adds bot state outside that directory.

## Identity

- **AgentDevice**: One per agent. Fields the process already has: `user_id` from the Matrix session, `device_id` from `CRYPTO_STORE_PATH/.device-id` after it matches `/whoami`. No second `device_id` field is introduced for the bot.
- **RetiredDevice**: The AgentZeroBot device id that `matrix-bot-rust` logged in. 003 does not delete it inside the binary. Cutover stops `run_bot` and leaves that device unused. A later operator step may delete it from the homeserver. This plan does not do that delete.

## Files

| Path | Contents | Lifetime |
|------|----------|----------|
| `/a0/usr/workdir/matrix-bot/room_contexts.json` | JSON object, `room_id` to Agent Zero `context_id` | Existing file. Reloaded at startup. Rewritten when a response returns a new context id |
| `/a0/usr/workdir/matrix-bot/processed_event_ids.json` | JSON array of event ids, newest at the end, length at most 1000 | Created by this feature. Survives restarts. Not a sync token |
| `/a0/usr/workdir/matrix-bot/sync_cursor.json` | Legacy `next_batch` plus event ids | Not read. Not written. `matrix-sdk` owns `next_batch` |
| `/a0/usr/workdir/matrix-bot/.a0-api-key` | Agent Zero credential, one line | Existing secret file. Read on startup and again after HTTP 401. Never logged |
| `CRYPTO_STORE_PATH/` | Unchanged 002 store | Still opened only by this process |

`room_contexts.json` values are opaque strings. Missing file means an empty map. A room with no entry sends `context_id` as an empty string on the first turn.

## Configuration

| Variable | Default | Meaning |
|----------|---------|---------|
| `BOT_ENABLED` | `false` | `true` registers invite and message handlers. Any other value leaves them off |
| `BOT_DISPLAY_NAME` | empty | Display name set at startup when non-empty. Also a trigger prefix when it differs from the short user name |
| `AGENT_IDENTITY` | `BOT_DISPLAY_NAME`, then `Agent Zero` | Name used in fast replies and the Agent Zero preface |
| `A0_API_URL` | `http://localhost:80/api/api_message` | Agent Zero message endpoint |
| `AMCTL_URL` | `http://172.23.88.254:8077` | am-ctl base URL |
| `AMCTL_API_KEY` | unset | Required for a live trusted-sender fetch. Absence is a failed fetch, not a built-in key |
| `TRUSTED_SENDERS` | unset | Comma-separated Matrix ids used only when the am-ctl fetch fails or returns an empty list |
| `BOT_STATE_DIR` | `/a0/usr/workdir/matrix-bot` | Directory for the two JSON files and `.a0-api-key` |

`GET /health` adds `botEnabled` (boolean). It does not add device secrets.

## Event decisions

Handled in order. The first match wins.

1. **Self**: `sender` equals the session user id. Record the event id. No reply.
2. **Duplicate**: event id already in the persisted list. No reply.
3. **Stale**: `now_ms - origin_server_ts > 300000`. No reply.
4. **Not text**: `type` is not `m.room.message` or `msgtype` is not `m.text`. Invite events are a separate path. No reply.
5. **UTD**: decryption failed. Structured warning with event id and reason. No reply.
6. **No trigger**: body, after mention-pill removal, does not start with a configured prefix. No reply.
7. **Fast command**: remainder is `ping`, `hello`, or `whoami`. Reply from the table in `contracts/bot-dispatch.md`. Do not call Agent Zero.
8. **Dispatch**: spawn the Agent Zero call. On success, send the rendered reply and update `room_contexts.json` when the context id changes.

Processed ids are appended for cases 1 through 8 once the event has been classified, including events that get no reply, so a restart does not reprocess them. The list drops from the front past 1000 entries.

## Invite

An `m.room.member` invite whose state key is this user:

- Inviter in the trusted set: join, then send the greeting once.
- Inviter absent, or sender unknown: log `reject_untrusted` with room id and sender. Do not join.

The trusted set is replaced only by a non-empty am-ctl response. A failed refresh keeps the previous set for the rest of the process, and uses `TRUSTED_SENDERS` only when the in-memory set is still empty.

## Reply payload

- **Fast reply**: plain `m.text` body from the contract. No HTML required.
- **Agent Zero reply**: Markdown rendered to HTML with `pulldown-cmark`. `room.send()` carries the plain text and the HTML. Chunks are at most 4000 characters of the plain text, split on a newline when one exists in the last 500 characters of the chunk.
- **Typing**: `typing: true` with a 60000 ms timeout before the Agent Zero call, then `typing: false` after the send or the error.

## Errors

- `BotDisabled`: internal state when `BOT_ENABLED` is not true. Handlers are not registered. Not an MCP tool error.
- `TrustedSenderUnavailable`: am-ctl failed and `TRUSTED_SENDERS` is empty. The invite is rejected.
- `AgentZeroAuth`: `/api/api_message` returned 401 after one key reload. Log the status only. Send no partial reply.
- `AgentZeroUnavailable`: the POST failed or timed out. Clear the typing notice. Send no fabricated answer.
