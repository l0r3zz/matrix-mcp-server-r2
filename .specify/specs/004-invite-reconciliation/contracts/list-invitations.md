# Contract: `list_invitations`

**Feature**: `004-invite-reconciliation` | **Tier**: read (tier0) | **Added in**: `v0.4.0`

## Input

```json
{}
```

There are no parameters. The tool reads the process's own Matrix user.

## Output

The result is one text content item that holds a JSON object. Other tools in `src/mcp/server.rs` return text the same way.

```json
{
  "count": 1,
  "truncated": false,
  "invitations": [
    {
      "roomId": "!abc:agent0-2.example",
      "name": "ops-room",
      "canonicalAlias": "#ops:agent0-2.example",
      "topic": "Ops chat",
      "inviter": "@operator:hub.example",
      "encryption": "encrypted",
      "trusted": "true",
      "attempts": 2,
      "lastError": "temporary 502"
    }
  ]
}
```

| Field | Type | Rule |
|-------|------|------|
| `count` | integer | The number of entries in `invitations` |
| `truncated` | boolean | True when more than 100 invites exist (FR-012) |
| `roomId` | string | Always present |
| `name` | string or null | `Room::name()`, then `cached_display_name()` |
| `canonicalAlias` | string or null | |
| `topic` | string or null | |
| `inviter` | string or null | Null when `invite_details()` fails |
| `encryption` | `"encrypted"`, `"unencrypted"`, or `"unknown"` | From `Room::encryption_state()` |
| `trusted` | `"true"`, `"false"`, or `"unknown"` | `"unknown"` when the reconciler is not active (FR-011) or has not decided this room yet |
| `attempts` | integer | 0 when the bot is disabled |
| `lastError` | string or null | `JoinError` display, never a server message body |

- Invites are sorted by `roomId`, so output order is stable.
- With no invites the result is a success: `{"count":0,"truncated":false,"invitations":[]}`.

## Errors

- `SKIP_MATRIX_INIT` stub client: a success with an empty list, the same as other read tools on the stub.
- A per-room read failure (for example, `invite_details`) fills that room's field with null. It does not fail the call.

## Never in output

- Room messages
- Access tokens
- Recovery keys
- `AMCTL_API_KEY`
- Agent Zero API keys
- Raw server error bodies

## Behavior that does not change

- `join_room` keeps its input, output, and error text (FR-013).
- Calling `list_invitations` never joins, declines, or retries.

## Reconciler contract (internal)

- It runs only when `BOT_ENABLED=true` and `bot::attach` registered its handlers.
- First pass: after `bot::attach` returns, before the first background sync wait.
- Later passes: after each successful `sync_once`, and when the earliest retry deadline passes.
- At most one pass at a time, and at most one join per room at a time.
- It is aborted on graceful shutdown.
