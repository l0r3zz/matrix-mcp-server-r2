# Data Model: Image Prompt

**Feature**: `005-image-prompt` | **Date**: 2026-11-25

No new file. No new MCP tool. No change to `room_contexts.json` or `processed_event_ids.json` beyond the event id this dispatcher already records for a handled message.

## ImageCapability

Operator switch for this process.

| Field | Source | Rule |
|-------|--------|------|
| `accepts_images` | `MODEL_ACCEPTS_IMAGES` | `true` only when the value is the string `true`. Every other value, including unset, is `false`. |

Copied from `Config` onto `bot::Session` and `dispatch::Runtime` at startup. It is not read again per message.

## Incoming image

Fields added to the event the handler already builds. Absent on a text prompt.

| Field | Rule |
|-------|------|
| `msgtype` | `m.image` for an image prompt. `m.text` is unchanged. |
| `body` | The event `body`. The trigger runs on this string. |
| `source` | The one `MediaSource` from `ImageMessageEventContent::source()`. Plain or encrypted. |
| `size` | `info.size` when present, else unknown until download. |
| `mimetype` | `info.mimetype` when present. |
| `extra_media_urls` | Count of non-thumbnail `mxc://` URLs besides `source`. Zero on an Element image. |

The thumbnail URL is not stored.

## Decision

`trigger::classify` gains one outcome and keeps the rest.

| Outcome | When |
|---------|------|
| Existing text outcomes | `m.text`, unchanged, including fast replies. |
| `DropNotText` | Not `m.text` and not `m.image`. |
| `NoTrigger` | `m.image` whose `body` does not address the agent. |
| `ImagePrompt { caption }` | `m.image` whose `body` matches a prefix. `caption` is the trimmed remainder and may be empty. Not a fast reply. |

`ImagePrompt` records the event id, same as `Dispatch`.

## PreparedImage

Built only after the capability, count, and declared-size checks pass. Dropped before `ask` on any later failure.

| Field | Rule |
|-------|------|
| `filename` | `image-<safe-id>.<ext>`. `<safe-id>` is derived from the event id. `<ext>` is `png`, `jpeg`, `webp`, `gif`, or `img`. |
| `bytes` | The downloaded file. Length 1 through 10_000_000. Not logged. Not written to `BOT_STATE_DIR`. |
| `base64` | Standard base64 of `bytes`, held only long enough to build the POST body. |

## Evaluator request

The JSON `agent_zero::ask` already posts, plus one optional field.

| Field | Text prompt | Image prompt |
|-------|-------------|--------------|
| `message` | `message_body(...)` | Same framing. `prompt` is the caption, which may be empty. |
| `context_id` | Current room context | Same. |
| `attachments` | Key omitted | One object, `{ "filename", "base64" }`. |

A refused image prompt does not build this request.

## Room error

Not stored. One plain-text room message from the table in `research.md` R8. A pre-send error does not update `room_contexts.json`. An evaluator failure after a sent image also leaves the saved context unchanged and posts `The model did not answer.`

## State transitions

```text
m.image event
  -> no trigger: stop
  -> triggered and capability false: room error, stop
  -> more than one image: room error, stop
  -> declared size over 10_000_000: room error, stop
  -> download or prepare fails, or bytes over 10_000_000: room error, stop
  -> POST once with message + one attachment
       -> one model reply, existing send path
       -> call fails or returns no answer: one room error "The model did not answer.", no second POST
```

A second image event uses this path on its own. It does not cancel the first.
