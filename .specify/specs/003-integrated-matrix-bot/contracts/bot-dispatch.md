# Contract: Bot Dispatch

**Date**: 2026-11-05
**Spec**: `.specify/specs/003-integrated-matrix-bot/spec.md`

This is the behavior of `src/bot/`. It is not an MCP tool. `spec/matrix-mcp-v1-v2.yml` does not change.

## Inputs the dispatcher accepts

A joined-room timeline event, or an invite.

| Field | Rule |
|-------|------|
| Room | Any room the shared client has joined, encrypted or not |
| Event type | `m.room.message` with `msgtype` `m.text`, or an invite membership event |
| Sender | Not this agent's user id |
| Age | `origin_server_ts` within 300000 ms of local receipt |
| Body | Mention pills removed. Must start with one prefix below. Match is case-insensitive |

Prefixes, built from `MATRIX` user id localpart `agent_short` and optional `BOT_DISPLAY_NAME`:

- `{agent_short}:`
- `{agent_short},`
- `@{agent_short}`
- `@all-agents:`
- `@all-agents,`
- `{display_name}:`, `{display_name},`, `@{display_name}` when the display name is non-empty and is not the same string as `agent_short`

The remainder after the prefix is trimmed. That remainder is the command or the Agent Zero prompt.

## Fast replies

No call to Agent Zero.

| Remainder | Body sent |
|-----------|-----------|
| `ping` | `PONG` |
| `hello` | `Hello! I'm {identity}, online and ready.` |
| `whoami` | `I'm {identity} ({user_id}).` |

`{identity}` is `AGENT_IDENTITY`. `{user_id}` is the session Matrix id.

## Agent Zero request

`POST {A0_API_URL}`

Headers: `Content-Type: application/json`, `X-API-KEY` set from `.a0-api-key`.

```json
{
  "message": "<preface plus the triggered text>",
  "context_id": "<value from room_contexts.json, or empty>"
}
```

The preface is the live instruction: identify as `{identity}` (`{user_id}`), reply with plain conversational text, and do not send messages via Matrix MCP tools. The triggered text keeps the sender and the room id on the following lines, as the current bot does.

A 401 re-reads `.a0-api-key` and retries the POST once. A later context id in the response replaces the map entry for that room.

## Room output

`room.send()` on the shared client. In an encrypted room the SDK encrypts. In an unencrypted room the body is plaintext. Fast replies are `m.text`. Agent Zero replies include HTML rendered by `pulldown-cmark`, in chunks of at most 4000 characters.

Join greeting, sent once after a trusted join:

`Hello! I'm {display_name}, an AI agent powered by Agent Zero.`

## Trusted-sender request

`GET {AMCTL_URL}/api/v1/trusted-senders`

Header: `X-API-Key: {AMCTL_API_KEY}`

Success body: a JSON array of objects with `matrix_user_id`. A non-empty array replaces the trusted set. Any other outcome uses `TRUSTED_SENDERS` when the in-memory set is empty, and otherwise keeps the previous set. An invite from outside the set is logged and not joined.

## Health

`GET /health` JSON gains:

```json
{ "botEnabled": false }
```

`true` only when `BOT_ENABLED=true` and the handlers were registered. Existing `e2ee` and `crossSigned` fields stay as 002 defined them.

## Out of this contract

- New or renamed MCP tools
- A second `/sync` loop or a second access token
- Reading `sync_cursor.json`
- Deleting the retired AgentZeroBot device on the homeserver
- Replying to `m.notice`, `m.emote`, or media messages
