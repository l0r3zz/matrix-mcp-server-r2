# Tasks: Image Prompt

**Input**: `.specify/specs/005-image-prompt/`
**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/image-prompt.md`, `quickstart.md`

Stories come from `spec.md`:

- **US1 (P1)**: Refuse an image the model cannot see
- **US2 (P1)**: Evaluate one image with the prompt
- **US3 (P2)**: Reject an image the agent will not evaluate

Tests are included because Constitution Principle VII requires them. Write each story's tests so they fail before that story's implementation.

No task hardcodes an instance number. Fixtures use `@agent0-5:example.com` and `@other:example.com`. The lab target is `@agent0-5`. Do not use `@agent0-2`.

Do not bump `version` in `Cargo.toml`. Do not add a crate other than `base64` 0.22.1. Do not open a second crypto store or start a second sync loop. Do not change `spec/matrix-mcp-v1-v2.yml`. Do not log image bytes, captions, filenames, base64, access tokens, recovery keys, or `AMCTL_API_KEY`.

Principle III applies to every task:

- Put a comment on each `.clone()`, including `Client`, `Room`, and `Arc` clones.
- Error types use `thiserror`.
- No `unwrap()` or `expect()` in production paths.
- Every public item has a doc comment.
- Do not hold the bot state lock across a media download or an evaluator POST.

## Phase 1: Setup

**Purpose**: The one new dependency, and the roadmap line.

- [X] T001 Add direct dependency `base64` version `0.22.1` in `Cargo.toml` and refresh `Cargo.lock` for that crate only. Use the standard engine, not the URL-safe engine. Leave `version` at `0.3.1`.
- [X] T002 [P] In `.specify/memory/constitution.md`, add `005-image-prompt` to the Principle IX roadmap list. Bump the constitution version from `2.1.0` to `2.2.0`, set Last Amended to the date of the edit, and add this amendment note: `Amendment 2.2.0 (MINOR): 005-image-prompt is on the roadmap. The 21 + 2 tool count and the locked stack table did not change.` Do not change any principle text other than that roadmap line.

## Phase 2: Foundational

**Purpose**: The operator switch every story reads. No image behavior yet.

- [X] T003 Add `MODEL_ACCEPTS_IMAGES` to `src/config.rs`. `accepts_images` is `true` only when the value is the string `true`. Unset, empty, `false`, and `1` are `false`. Copy it onto `Session` in `src/bot/mod.rs`, onto `Runtime` in `src/bot/dispatch.rs`, and pass it from `src/main.rs`.

**Checkpoint**: `cargo test` passes. A text prompt is unchanged. No image is downloaded.

## Phase 3: User Story 1 - Refuse an image the model cannot see (Priority: P1)

**Goal**: A triggered image posts one room error and is not sent when the model cannot take images, or when the server has not been told that it can (FR-003, FR-004, SC-001). A text prompt still gets an answer (FR-002, SC-002).

**Independent Test**: Leave `MODEL_ACCEPTS_IMAGES` unset. Send one triggered image. The room shows one error and the evaluator is not called. A triggered text prompt still gets an answer.

### Tests for User Story 1

- [X] T004 [P] [US1] Add unit tests in `src/bot/trigger.rs`:
  - An `m.image` body that does not match a prefix returns `NoTrigger`.
  - An `m.image` body that matches a prefix returns `ImagePrompt` with the trimmed remainder, including an empty remainder.
  - `ping`, `hello`, and `whoami` on an `m.image` are `ImagePrompt`, not a fast reply.
  - `m.file`, `m.video`, and `m.audio` stay `DropNotText`.
  - An `m.text` prompt is unchanged, including fast replies.
  - An `m.image` event with `decrypt_error` set returns `UnableToDecrypt`, including when `msgtype` is `m.image` and the body addresses the agent.
- [X] T005 [P] [US1] Add `tests/bot_image_prompt.rs` with wiremock. One `m.image` event, `MODEL_ACCEPTS_IMAGES` unset:
  - The room send is plain text `This agent cannot evaluate an image. The prompt was not evaluated.`
  - There is no POST to the evaluator.
  - There is no media download.
  - A triggered `m.text` prompt still POSTs JSON with `message` and `context_id` and no `attachments` key.
  - Logged lines do not contain the test access token or the image body.

### Implementation for User Story 1

- [X] T006 [US1] In `src/bot/mod.rs` and `src/bot/trigger.rs`, keep one `m.image` through the plain handler and the decrypted handler. `ImagePrompt.caption` is the trimmed remainder and may be empty. Do not download. Do not treat the thumbnail as an image. Leave the decrypted handler behind the existing `e2ee` cfg. A `decrypt_error` stays `UnableToDecrypt` and is handled before any image branch. Do not post a room error for it.
- [X] T007 [US1] In `src/bot/dispatch.rs`, when `accepts_images` is `false`, post one `text_plain` room message with `This agent cannot evaluate an image. The prompt was not evaluated.` Log `room_id`, `event_id`, and `refuse_capability`. Do not download. Do not call `agent_zero::ask`.

**Checkpoint**: US1 tests pass. An unset flag refuses a triggered image. Text prompts are unchanged.

## Phase 4: User Story 2 - Evaluate one image with the prompt (Priority: P1)

**Goal**: When the operator has said the model can take images, one ready image and its caption go together on the existing evaluation, once, and the room gets one answer (FR-006, FR-007, SC-003, SC-004). If that call fails or returns no answer, the room gets one error and the caption is not sent again (FR-015, SC-008).

**Independent Test**: Set `MODEL_ACCEPTS_IMAGES=true`. Send one small triggered image a tester can identify. The evaluator POST contains the caption and that one image. The room shows one answer. A failed evaluator call shows `The model did not answer.` and does not POST again.

### Tests for User Story 2

- [X] T008 [P] [US2] Add unit tests in `src/bot/agent_zero.rs`:
  - A text turn serializes `message` and `context_id` and omits `attachments`.
  - An image turn adds `attachments` with one object, keys `filename` and `base64`.
  - `filename` matches `image-<safe-id>.<ext>`. `<ext>` is `png`, `jpeg`, `webp`, `gif`, or `img`. The safe id uses only ASCII letters, digits, `.`, `_`, and `-`.
  - `base64` is standard base64 of the bytes, with padding.
  - An empty caption still produces a non-empty `message` through `message_body`. The image bytes are not inside `message`.
- [X] T009 [P] [US2] Extend `tests/bot_image_prompt.rs`. `MODEL_ACCEPTS_IMAGES=true`, one plain `m.image` at or under 10_000_000 bytes:
  - One evaluator POST has `message`, `context_id`, and one `attachments` entry.
  - The room receives one model reply on the existing HTML path.
  - A failed evaluator response posts plain text `The model did not answer.` and does not POST a second time.
  - A failed text prompt posts no new room error.
  - Logged lines do not contain the test token or the base64 payload.

### Implementation for User Story 2

- [X] T010 [US2] In `src/bot/agent_zero.rs`, add the optional `attachments` array from `contracts/image-prompt.md` to the existing POST. Encode with `base64` 0.22.1, standard engine. Omit the key when there is no image.
- [X] T011 [US2] In `src/bot/dispatch.rs`, download with the shared client's `media().get_media_content`, `MediaFormat::File`, and `use_cache` `false`. The source is `ImageMessageEventContent::source()`. Call `ask` once, and only after one image was downloaded, is non-empty, and is at or under 10_000_000 bytes. On success, send the reply as today. On a failed call or an empty answer, post `The model did not answer.` and do not call `ask` again. If the download fails, the bytes are empty, or the byte length is greater than 10_000_000, do not call `ask`. The room text for that stop is T014. Log `sent` or `evaluator_failed` with `room_id` and `event_id` only.

**Checkpoint**: US2 tests pass in both feature builds. A plain image works without `--features e2ee`. A failed or oversized download does not call `ask`. This checkpoint is not a release until T014 supplies the room errors. Encrypted-file download stays a lab check.

## Phase 5: User Story 3 - Reject an image the agent will not evaluate (Priority: P2)

**Goal**: More than one image, an image over 10_000_000 bytes, or an image that cannot be prepared posts one room error and does not send the caption (FR-008, FR-009, FR-010, SC-005).

**Independent Test**: With `MODEL_ACCEPTS_IMAGES=true`, one event with two non-thumbnail media URLs, one image whose `info.size` is greater than 10_000_000, and one image whose download fails each produce one room error and no evaluator POST.

### Tests for User Story 3

- [X] T012 [P] [US3] Add unit tests in `src/bot/dispatch.rs`:
  - Two different non-thumbnail `mxc://` URLs in one content object are more than one image.
  - A thumbnail URL does not count as a second image.
  - `info.size` greater than 10_000_000 bytes refuses before download.
  - Downloaded length greater than 10_000_000 bytes refuses with `The image is too large. The prompt was not evaluated.`
  - Empty bytes refuse.
  - A declared type other than `image/*` refuses. A missing type does not refuse by itself.
- [X] T013 [P] [US3] Extend `tests/bot_image_prompt.rs`:
  - Declared size greater than 10_000_000 bytes makes no media GET and no evaluator POST. The room text is `The image is too large. The prompt was not evaluated.`
  - Two non-thumbnail media URLs make no evaluator POST. The room text is `Only one image is supported. The prompt was not evaluated.`
  - A failed media GET makes no evaluator POST. The room text is `The image could not be read. The prompt was not evaluated.`

### Implementation for User Story 3

- [X] T014 [US3] In `src/bot/dispatch.rs`, run the checks in this order and stop at the first failure. Do not call `agent_zero::ask`.
  1. More than one non-thumbnail media URL. Room text: `Only one image is supported. The prompt was not evaluated.` Log `refuse_count`. Do not download.
  2. `info.size` present and greater than 10_000_000 bytes, or the downloaded length is greater than 10_000_000 bytes. Room text: `The image is too large. The prompt was not evaluated.` Log `refuse_size`. Do not download when `info.size` is already over the limit. When the downloaded length is over the limit, drop the bytes.
  3. Download fails, the bytes are empty, or the declared type is present and is not `image/*`. Drop the bytes. Room text: `The image could not be read. The prompt was not evaluated.` Log `refuse_unreadable`.

**Checkpoint**: US1, US2, and US3 tests pass in both feature builds.

## Phase 6: Polish

- [X] T015 [P] Update `src/AGENTS.md` with `MODEL_ACCEPTS_IMAGES`, the one-image path, the refuse-before-call rule, and the room error `The model did not answer.` Do not change the `21 + 2` tool count.
- [X] T016 [P] Document `MODEL_ACCEPTS_IMAGES` in `.env.example`. State that only the string `true` allows an image prompt.
- [X] T017 Run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features e2ee -- -D warnings`, `cargo test`, and `cargo test --features e2ee`.
- [X] T018 Run the `@agent0-5` checks in `quickstart.md` only after the user approves the deploy. Recorded on 2026-11-26 in `spec/T018-Agent0-5-Result-20261126.md`. All eight lab checks passed. The quickstart automated list includes the failed-evaluator error and the three pre-send errors. The lab steps include the stopped-evaluator error, a second image message as its own prompt, and the oversize error. The two-image error is the automated check for one message with two non-thumbnail media URLs. Do not upgrade any other agent. Do not bump `Cargo.toml`. Do not tag or push until the user asks.

## Dependencies

- Phase 1, then Phase 2, then the story phases.
- US1 depends on T003.
- US2 depends on US1 (T006 and T007) and on T001.
- US3 depends on US2 (T011), because the size and download checks run only after the capability flag allows an image.
- Phase 6 depends on all stories.

## Parallel Opportunities

- T002 with T001.
- T004 and T005.
- T008 and T009.
- T012 and T013.
- T015 and T016.

US2 cannot start before US1, because both edit `src/bot/dispatch.rs` and US2 uses the `m.image` parse from US1.

## Implementation Strategy

1. **MVP**: Phases 1 to 3 (US1). A triggered image is no longer dropped. The sender sees one error, and the caption is not evaluated.
2. **Second increment**: Phase 4 (US2). One ready image is sent with the caption. A failed call tells the room the model did not answer. A failed download does not call `ask`. This increment is not released until Phase 5 enforces the one-image rule and the size limit.
3. **Third increment**: Phase 5 (US3). Count, size, and unreadable images refuse before the call.
4. **Release**: Phase 6. The `@agent0-5` lab check passed on 2026-11-26. `Cargo.toml` is `0.4.0`.
