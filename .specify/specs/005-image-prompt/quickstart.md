# Quickstart: Image Prompt

**Feature**: `005-image-prompt` | **Date**: 2026-11-25
**Lab target**: `@agent0-5`
**Contract**: `contracts/image-prompt.md`

Do not deploy this until the user asks. Do not bump `Cargo.toml`. Do not use `@agent0-2` for this feature.

## Automated checks

From the repo root, both builds:

```bash
cargo test
cargo test --features e2ee
```

The tests that belong to this feature:

- Unit: an `m.image` body with no prefix is ignored. A prefix with an empty remainder is an image prompt, not a fast reply. `ping` on an image is not a fast reply. Two non-thumbnail media URLs refuse. A thumbnail does not count. Size above 10_000_000 bytes refuses before download. A text prompt is unchanged.
- Wiremock: one plain `m.image` with `MODEL_ACCEPTS_IMAGES` unset posts the capability error and does not `POST` to the evaluator. The same image with the flag `true` and a media body at or under the limit posts one JSON body with `message` and one `attachments` entry. The logged lines do not contain the test token or the base64 payload.
- Wiremock, flag `true`: a failed evaluator response posts `The model did not answer.` and does not POST again. A failed text prompt posts no new room error.
- Wiremock, flag `true`: declared size greater than 10_000_000 bytes does not download and does not POST, and the room text is `The image is too large. The prompt was not evaluated.` Two non-thumbnail media URLs do not POST, and the room text is `Only one image is supported. The prompt was not evaluated.` A failed media GET does not POST, and the room text is `The image could not be read. The prompt was not evaluated.`
- A text `POST` from an existing bot test still has no `attachments` key.

Encrypted-file download is not required in the wiremock test. The lab checks below cover an encrypted Element room on a binary built with `--features e2ee`.

## Lab checks

Prerequisite: `BOT_ENABLED=true`, the bot handlers registered, and `/health` healthy. The evaluator is the Agent Zero already on that agent. No fleet image change.

1. Leave `MODEL_ACCEPTS_IMAGES` unset. From Element, send one image whose caption addresses `@agent0-5`. The room shows `This agent cannot evaluate an image. The prompt was not evaluated.` No model answer follows. A text prompt in the same room still gets one answer.
2. Set `MODEL_ACCEPTS_IMAGES=true` and restart the process. Send one small image whose picture a person can identify, with a caption that addresses the agent. The room shows one model answer about that picture.
3. Repeat step 2 in an unencrypted room and in an encrypted room. Each room gets one answer.
4. Send an image whose caption does not address the agent. The room stays quiet.
5. Send a text prompt. The answer path is unchanged.
6. With the flag still `true`, send one triggered image while the evaluator is stopped. The room shows `The model did not answer.` The caption is not sent again. Start the evaluator before the next step.
7. Send a second triggered image as its own message. The room treats it as a separate prompt and shows one answer for that image. This version is one image per message. The error `Only one image is supported. The prompt was not evaluated.` is the automated check for one message with two non-thumbnail media URLs.
8. Send one triggered image larger than 10_000_000 bytes. The room shows `The image is too large. The prompt was not evaluated.`

Stop after a failed step. Do not upgrade any other agent.
