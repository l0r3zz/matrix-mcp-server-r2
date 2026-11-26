# Contract: Image Prompt

**Date**: 2026-11-25
**Spec**: `.specify/specs/005-image-prompt/spec.md`

This is the behavior of `src/bot/` for one image on a triggered prompt. It is not an MCP tool. `spec/matrix-mcp-v1-v2.yml` does not change.

## Config

| Variable | Rule |
|----------|------|
| `MODEL_ACCEPTS_IMAGES` | The string `true` allows an image prompt. Any other value, including unset, refuses. |
| `BOT_ENABLED` | Unchanged. When it is not `true`, this contract does not run and posts no error. |

## Event

| Field | Rule |
|-------|------|
| Type | One `m.room.message` |
| `msgtype` | `m.image` |
| Body | Same prefixes as a text prompt. A body that does not match is ignored. |
| Sources | One image source. The thumbnail does not count. Two non-thumbnail media URLs in the same event are an error. |
| Other msgtypes | Unchanged. No new error and no new success path. |
| Undecryptable event | Unchanged. Log only. No new room error. |

A triggered image is not a fast reply. This version is one image per message. A second message with its own image is a separate prompt.

## Room errors

One plain-text message. The caption is not sent to the evaluator.

| Condition | Text |
|-----------|------|
| `MODEL_ACCEPTS_IMAGES` is not `true` | `This agent cannot evaluate an image. The prompt was not evaluated.` |
| More than one image in the event | `Only one image is supported. The prompt was not evaluated.` |
| Declared or downloaded size greater than 10_000_000 bytes | `The image is too large. The prompt was not evaluated.` |
| The picture cannot be downloaded, is empty, has a declared type other than `image/*`, or cannot be encoded | `The image could not be read. The prompt was not evaluated.` |
| The image was sent, and the call fails or returns no answer | `The model did not answer.` |

The error is not the HTML model reply. It contains no image bytes, tokens, or keys.

## Evaluator request

`POST` to the existing `A0_API_URL` with the existing `X-API-KEY` header.

Text prompt, unchanged:

```json
{
  "message": "<message_body>",
  "context_id": "<context id or empty>"
}
```

Image prompt, only after the image is ready:

```json
{
  "message": "<message_body with the caption>",
  "context_id": "<context id or empty>",
  "attachments": [
    {
      "filename": "image-<safe-id>.<ext>",
      "base64": "<standard base64>"
    }
  ]
}
```

One attachment. One POST. A successful reply uses the current HTML send path. A failed POST, or a POST that returns no answer, posts `The model did not answer.` and does not send the caption again. A text prompt that fails this way stays on its current path and posts no new error.

## Logs

A handled image logs `room_id`, `event_id`, and one of `refuse_capability`, `refuse_count`, `refuse_size`, `refuse_unreadable`, `sent`, `evaluator_failed`. Nothing else from the image or the caption.
