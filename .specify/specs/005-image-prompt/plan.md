# Implementation Plan: Image Prompt

**Branch**: `005-image-prompt` | **Date**: 2026-11-25 | **Spec**: `.specify/specs/005-image-prompt/spec.md`

**Input**: Feature specification from `.specify/specs/005-image-prompt/spec.md`

## Summary

A triggered Element image is dropped today because the bot keeps only `m.text`. This feature keeps one `m.image` on that same handler. When `MODEL_ACCEPTS_IMAGES` is not `true`, the room gets one plain-text error and the caption is not sent. When the flag is `true`, the shared client downloads the one image, including an encrypted file, and the existing Agent Zero `POST` gains one `attachments` entry. Agent Zero v2.14 already accepts that entry. The fleet repo does not change. A bad or missing image never becomes a text-only call, because Agent Zero would answer the caption anyway. A sent image whose call fails or returns no answer posts one plain-text error, `The model did not answer.`, and does not retry. A failed text prompt stays quiet.

## Technical Context

**Language/Version**: Rust 1.99, edition 2021 (constitution pin)

**Primary Dependencies**: Existing `matrix-sdk` 0.19.1 (`Client::media`, `MediaRequestParameters`, `MediaSource`, `ImageMessageEventContent`) and `ruma` 0.17. One new direct dependency: `base64` 0.22.1, already in `Cargo.lock`. No other crate. No stack bump.

**Storage**: No new file. Image bytes live only for the download and the POST body. `use_cache` is `false`. `room_contexts.json` updates only after a successful evaluator reply, as it does today.

**Testing**: `cargo test` and `cargo test --features e2ee`. Unit tests for classify, the size and count checks, and the POST JSON. Wiremock for one plain `m.image`, the media bytes, and the evaluator POST. The encrypted Element room is a lab check in `quickstart.md`.

**Target Platform**: Agent-Matrix container, the same binary at `/a0/usr/workdir/matrix-mcp-server/`.

**Project Type**: The existing Axum process. This feature is the in-process `src/bot/` dispatcher. No new MCP tool.

**Performance Goals**: A refused image posts its room error within 30 seconds, with no media download when the flag is off (SC-001). A ready image at or under 10_000_000 bytes produces one room answer within 2 minutes (SC-003).

**Constraints**:

- One Matrix client and one crypto store (Principle IV). Download goes through `matrix-sdk`.
- No image bytes, caption, base64, token, or key in logs (Principle V).
- Refuse before `agent_zero::ask` whenever the image will not be in that call.
- 10 megabytes means 10_000_000 bytes.
- `MODEL_ACCEPTS_IMAGES` defaults to off.
- Both Cargo feature builds pass. A plain image works without `e2ee`. Encrypted media needs the `e2ee` build, same as encrypted text today.
- `Cargo.toml` `version` stays `0.3.1`.

**Scale/Scope**: One image per message. A second message is a separate prompt. One message with two non-thumbnail media URLs is the automated one-image error. One agent process. The lab target is `@agent0-5`.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (Unified Tool Surface)**: No tool is added or renamed. `spec/matrix-mcp-v1-v2.yml` stays as it is. **Pass.**
- **Principle II (Feature-Gated E2EE)**: The image decision and the plain download compile in both builds. Encrypted event decryption stays behind `#[cfg(feature = "e2ee")]`, which is the 003 boundary. **Pass.**
- **Principle III (Rust Idioms)**: New errors use `thiserror`. No `unwrap` or `expect` on the new path. Clones of `Room` and `Arc` get a comment. The new public items get doc comments. The state lock is not held across the download or the POST. **Pass.**
- **Principle IV (Single Store Owner)**: `get_media_content` uses the shared client. No second store and no local decrypt. **Pass.**
- **Principle V (Security First)**: Logs are `room_id`, `event_id`, and a class token. The room error has no bytes or secrets. The base64 exists only in the POST body. **Pass.**
- **Principle VI (Transparent Encryption)**: Plain and encrypted images use one `MediaSource` download. An event that cannot be decrypted keeps the current log-only path. **Pass.**
- **Principle VII (Test Everything)**: Unit tests plus a wiremock homeserver and a wiremock evaluator. CI runs both feature builds. **Pass.**
- **Principle VIII (Container-Aware)**: The switch is an environment variable. `/health` is unchanged. Nothing new is required on the crypto-store volume. **Pass.**
- **Principle IX (Incremental Delivery)**: **Amendment required.** The roadmap list ends at `004-invite-reconciliation`. Tasks add `005-image-prompt` to that list, bump the constitution from `2.1.0` to `2.2.0`, and record the amendment. The feature stays off unless `BOT_ENABLED=true`, and image evaluation stays off unless `MODEL_ACCEPTS_IMAGES=true`. **Pass with that amendment.**
- **Principle X (Spec-Driven)**: This plan, `research.md`, `data-model.md`, `contracts/image-prompt.md`, and `quickstart.md` are the design. The MCP YAML is unchanged because there is no tool change. **Pass.**

Gate result: PASS. The constitution edit is the Principle IX roadmap line plus the `2.2.0` amendment record, and it belongs to tasks, not to this plan.

Post-design re-check: PASS. The contract adds one optional JSON field on a POST this server already makes. The data model adds no file and no tool. No locked stack version changes. `base64` 0.22.1 does not bump the locked table.

## Project Structure

### Documentation (this feature)

```text
.specify/specs/005-image-prompt/
├── spec.md
├── plan.md                    # this file
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── image-prompt.md
├── checklists/
│   └── requirements.md
└── tasks.md                   # created by /speckit-tasks
```

### Source Code

```text
src/config.rs                # MODEL_ACCEPTS_IMAGES, true only for the string "true"
src/bot/mod.rs               # Session field; m.image kept on the plain and decrypted paths
src/bot/trigger.rs           # ImagePrompt outcome; image is not a fast reply
src/bot/dispatch.rs          # refuse or download, then one ask
src/bot/agent_zero.rs        # optional attachments on the existing POST
src/main.rs                  # copy the flag into Session
tests/bot_invites.rs         # unchanged invite tests
tests/                       # new or extended bot test for the image POST
.specify/memory/constitution.md  # tasks add 005 to the Principle IX list and bump to 2.2.0
src/AGENTS.md                # tasks record the flag and the image path
```

**Structure Decision**: One crate. The image stays inside `src/bot/` next to the text prompt. No new module unless `dispatch.rs` becomes unwieldy during tasks. The MCP server and `spec/matrix-mcp-v1-v2.yml` stay untouched.

## Complexity Tracking

| Decision | Why Needed | Simpler Alternative Rejected Because |
|----------|------------|-------------------------------------|
| Direct `base64` 0.22.1 dependency | The evaluator contract is standard base64, and `Cargo.toml` has no encoder | A hand-rolled encoder can omit padding. Agent Zero then drops the attachment and answers the caption |
| `use_cache: false` | The picture is needed once for the POST | The media cache writes the bytes into the store with no later reader in this feature |
| Capability checked before download | A refused prompt must not fetch or hold the image | Downloading first spends the media request and keeps bytes that will be discarded |
| Unique attachment filename | Agent Zero writes the file under the uploads directory by name | The sender's filename collides across prompts and can contain characters the evaluator rejects |
| Constitution IX line deferred to tasks | The roadmap list is the delivery record | Editing the constitution inside the plan hides the amendment from the task list |
