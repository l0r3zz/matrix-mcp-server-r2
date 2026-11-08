# Quickstart: Prove the Integrated Bot

**Date**: 2026-11-05
**Spec**: `.specify/specs/003-integrated-matrix-bot/spec.md`

Lab proof for SC-001, SC-002, and SC-003. Run it on agent0-5 after `tasks.md` is implemented. Do not start this against a container that still has `run_bot` running.

## Prerequisites

- `matrix-mcp-server-r2` built with `--features e2ee` and already cross-signed on agent0-5 (feature 002).
- `CRYPTO_STORE_PATH` points at the existing store. `.device-id` matches `/whoami` for that token.
- `supervisord` program `run_bot` is stopped. `BOT_ENABLED=true`.
- `room_contexts.json` is the file already under `/a0/usr/workdir/matrix-bot/`.
- `.a0-api-key` is present and is the credential Agent Zero accepts.
- `AMCTL_URL` and `AMCTL_API_KEY` can read `/api/v1/trusted-senders`, and `@admin:v-site.net` is in that list.
- Two rooms where `@agent0-5` is already joined: Fleet HQ Clean (unencrypted) and one encrypted room.

## Build checks

From the repo root, before the lab:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features e2ee -- -D warnings
cargo test
cargo test --features e2ee
```

`GET /health` on the container reports `botEnabled: true` plus the existing `e2ee` and `crossSigned` booleans.

## SC-001 Fast commands

In Fleet HQ Clean, send `@agent0-5: ping` and `@agent0-5: hello`.

In the encrypted room, send the same two lines.

Expected: each room gets one reply from the agent0-5 device. `ping` returns `PONG`. `hello` returns `Hello! I'm {identity}, online and ready.` No second reply from a `matrix-bot-rust` process. `run_bot` stays stopped.

## SC-002 A real question

In the encrypted room, send `@agent0-5:` plus a short question.

Expected: Element shows one encrypted reply that renders as text. A second question in the same room uses the same Agent Zero context. `room_contexts.json` has an entry for that room id after the first answer. The reply is readable in Element on a cross-signed device.

## SC-003 Invites

From `@admin:v-site.net`, invite `@agent0-5` to a new room.

Expected: the agent joins and posts `Hello! I'm {display_name}, an AI agent powered by Agent Zero.`

From a Matrix id that is not in the trusted-sender list, invite `@agent0-5` to a different room.

Expected: the agent does not join. The process log contains `reject_untrusted` and the sender id. The log does not contain the API key or the access token.

## Failure signs

- Two replies to one `ping`: `run_bot` is still running, or Agent Zero called `send-message` itself.
- Encrypted `ping` with no reply, and a UTD warning for that event id: this device is not the cross-signed store device, or the sender is not cross-signed.
- `KeyStoreUnavailableError` at startup: a second process opened `CRYPTO_STORE_PATH`.
