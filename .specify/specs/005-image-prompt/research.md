# Research: Image Prompt

**Feature**: `005-image-prompt` | **Date**: 2026-11-25

## R1. Where an Element image is dropped today

**Decision**: Teach the existing message handler to keep an `m.image` event. Do not add a second handler.

**Rationale**: `register_message_handler` in `src/bot/mod.rs` keeps `MessageType::Text` and sets every other type, including `MessageType::Image`, to `msgtype: None`. `trigger::classify` then returns `DropNotText` (`src/bot/trigger.rs`). The encrypted path in `owned_from_raw` already copies the decrypted `msgtype` string, so an encrypted `m.image` also dies in `classify` because it is not `m.text`. One parse change covers both rooms.

**Alternatives considered**: A new event handler beside the text handler. It would race the existing handler on the same event and could answer twice.

## R2. What counts as one image

**Decision**: One `m.room.message` event with `msgtype` `m.image` and one `ImageMessageEventContent::source()` is one image. The thumbnail is not a second image. `m.file`, `m.video`, `m.audio`, and a text event with no image stay on the current path.

**Rationale**: Element sends one `m.image` event for one attached picture. The caption is `body` when the sender typed one. A filename-only `body` does not match a trigger, so it is ignored (spec assumption). `MediaEventContent::thumbnail_source` for an image falls back to the main source, so counting thumbnail plus source would reject every image that has no separate thumbnail.

A content object is "more than one image" only when it contains two different media URLs that are not the thumbnail URL. A normal Element image has one. The unit fixture for that error puts a second non-thumbnail `mxc://` URL in the same content object. A second Element image is a second event and is handled on its own.

**Alternatives considered**: Treating an Element album of several events as one prompt. The spec says a client that splits the caption and the image into two messages is out of scope. Each event keeps its own decision.

## R3. Operator switch

**Decision**: `MODEL_ACCEPTS_IMAGES=true` is the only value that allows an image prompt. Unset, empty, `false`, and `1` all refuse. The flag is copied onto `Session` and `Runtime` the same way `BOT_ENABLED` is. This server does not read the fleet `vision` flag.

**Rationale**: The spec says unset means refuse, and the server must not guess. `BOT_ENABLED` already treats only the string `true` as on. The fleet `vision` value lives in the Agent Zero preset and is not on this process.

**Alternatives considered**: Calling am-ctl to read `vision`. That adds a network dependency and a second source of truth. The spec leaves that flag unused.

## R4. Refuse before the evaluator call

**Decision**: For a triggered image, run the checks in this order. Stop at the first failure. Post one plain-text room message. Do not call `agent_zero::ask`.

1. Capability is not `true`. Do not download.
2. More than one non-thumbnail media URL. Do not download.
3. `info.size` is present and greater than 10_000_000 bytes. Do not download.
4. Download fails, the bytes are empty, or the declared type is present and is not `image/*`. Drop the bytes. A downloaded length greater than 10_000_000 bytes also stops the call, and its room text is the too-large error from R8.
5. The attachment object cannot be built. Do not send the caption alone.

**Rationale**: Agent Zero v2.14 `api/api_message.py` still runs the text when an attachment is skipped (`continue` on a bad attachment). A call that contains only the caption violates FR-004 and FR-010. The 10 megabyte limit in the spec is 10_000_000 bytes.

**Alternatives considered**: Download first, then check capability. That spends a media fetch and holds image bytes for a prompt that will not be sent.

## R5. Download and decrypt

**Decision**: Download with the shared client's `media().get_media_content(&MediaRequestParameters { source, format: MediaFormat::File }, false)`. `source` is `ImageMessageEventContent::source()`. `use_cache` is `false`.

**Rationale**: matrix-sdk 0.19.1 decrypts an encrypted file inside `get_media_content` when encryption is enabled. Plain and encrypted sources use the same call, which is FR-007. The shared client owns the crypto store (Principle IV). Skipping the cache avoids writing the picture into the media store for a one-shot prompt.

An encrypted room event still goes through `on_encrypted` first. If decryption of the event fails, the existing `UnableToDecrypt` path logs and does not post. This feature does not add a room error for that case (FR-013). A build without `--features e2ee` has no encrypted handler, which is the same as a text prompt today. A plain `m.image` works in both builds.

**Alternatives considered**: A manual `GET` of the `mxc://` URL plus a local Megolm decrypt. That is custom cryptography and violates Principle IV.

## R6. The evaluator body

**Decision**: Extend the existing `POST` JSON. Keep `message` and `context_id`. Add `attachments` only when one image is ready:

```json
{
  "message": "<existing message_body>",
  "context_id": "<existing context id>",
  "attachments": [
    { "filename": "image-<safe-id>.<ext>", "base64": "<standard base64 of the bytes>" }
  ]
}
```

`message` stays `message_body(...)`. An empty caption still produces a non-empty `message` because of that framing, which satisfies Agent Zero's "Message is required" check. The filename uses a short id derived from the event id and an extension from the media type (`png`, `jpeg`, `webp`, `gif`, otherwise `img`). Characters are limited to ASCII letters, digits, `.`, `_`, and `-`. The image bytes are not copied into `message`.

A text prompt sends the JSON it sends today, with no `attachments` key.

**Rationale**: This is the body Agent Zero v2.14 and the local `agent0ai/agent-zero` image at git tag `v1.9` already accept. The fleet repo does not need a change. A stable filename avoids two prompts overwriting `/a0/usr/uploads/<original name>`.

**Alternatives considered**: Putting a `data:` URL in the prompt text. The evaluator would not see a file, and the text would contain the image.

## R7. Encoding dependency

**Decision**: Add a direct `base64` dependency at 0.22.1, the version already in `Cargo.lock`. Use the standard engine, not URL-safe.

**Rationale**: The evaluator contract is standard base64. No direct dependency in `Cargo.toml` exposes an encoder. `base64` 0.22.1 is already in the lockfile through the existing tree.

**Alternatives considered**: Hand-rolled encoding. It is easy to get the padding wrong, and the refuse-before-call rule then hides the bug as a text-only answer inside Agent Zero.

## R8. Room error text

**Decision**: One `text_plain` send, not the HTML model-reply path.

| Cause | Plain text |
|-------|------------|
| Capability off or unset | `This agent cannot evaluate an image. The prompt was not evaluated.` |
| More than one image | `Only one image is supported. The prompt was not evaluated.` |
| Larger than 10_000_000 bytes | `The image is too large. The prompt was not evaluated.` |
| Download, type, or prepare failure | `The image could not be read. The prompt was not evaluated.` |
| Sent image, call fails or returns no answer | `The model did not answer.` |

**Rationale**: The spec says the error must not look like a model answer. Fast replies already use `RoomMessageEventContent::text_plain`. The model reply stays on `send_reply`.

## R9. Fast replies and an image

**Decision**: A triggered `m.image` is never a fast reply. `ping`, `hello`, and `whoami` in the caption do not skip the image.

**Rationale**: A fast reply would drop the image with no error. The spec says a triggered image is an image prompt.

## R10. Logging

**Decision**: Log `room_id`, `event_id`, and a class token (`refuse_capability`, `refuse_count`, `refuse_size`, `refuse_unreadable`, `sent`, `evaluator_failed`). Do not log the caption, the filename, the base64, the bytes, the access token, or the API key. A sent image whose call fails or returns no answer posts `The model did not answer.` and does not retry. A failed text prompt stays quiet, as it does today.

**Rationale**: Principle V forbids decrypted content and secrets at info level. The class token is enough to see which check fired.
