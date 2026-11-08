# Feature Specification: Integrated Reactive Matrix Bot Dispatcher (`src/bot/`)

**Feature Branch**: `003-integrated-matrix-bot` (Stage 2 following `002-e2ee-cross-signing`)
**Created**: 2026-10-29
**Status**: Planned (Stage 2 of Unified Gateway Architecture)
**Depends On**: `.specify/specs/002-e2ee-cross-signing/spec.md` (`matrix-sdk` 0.19, persistent SQLite crypto store, cross-signing auto-bootstrap)
**Input**: Consolidate `matrix-bot-rust` directly into `matrix-mcp-server-r2` (`Option 2` from `spec/Matrix-Bot-vs-MCP-Server-Consolidation-Analysis-20261029.md`) so all inbound and outbound Matrix traffic for an Agent-Matrix triplet flows through a single process, single `matrix-sdk` 0.19 client, single `.device-id`, and single SQLite crypto store.

## Clarifications

### Session 2026-11-05

- Q: How many Matrix devices does one Agent-Matrix agent keep after `matrix-bot-rust` is absorbed? → A: One. That device is the one already stored in this process's `CRYPTO_STORE_PATH` and `.device-id`. The separate AgentZeroBot device is retired. Do not mint a replacement device with `--logout` or a password reset.
- Q: If the old `run_bot` process is still running, should the new dispatcher answer Matrix messages too? → A: No. The dispatcher stays off unless `BOT_ENABLED=true`. The default is false. The operator stops `run_bot` first. This process does not stop `run_bot`.
- Q: When a triggered message needs an Agent Zero answer, who posts that answer into the Matrix room? → A: Only `src/bot/` posts it, via `room.send()`. The text sent to Agent Zero tells it not to use Matrix MCP tools for that turn.
- Q: If the trusted-sender lookup fails and the fallback list is empty, should the agent join the invite? → A: No. Log the room id and the sender. Do not join. No user id and no API key are compiled into the binary. A non-empty `TRUSTED_SENDERS` list is the only fallback when am-ctl cannot be read.
- Q: When the bot cannot decrypt an encrypted message, what should it do? → A: Log the event id and the reason. Do not post in the room. The log line contains no message body. History reads keep the existing unable-to-decrypt marker.
- Q: Which agent is the default implementation target? → A: None. Triggers come from the process's own Matrix user id, written `@agent0-N`. No function hardcodes `@agent0-1`. A lab proof that needs one concrete agent uses `@agent0-5`.

## Role & Motivation

In the legacy Agent-Matrix container layout, two separate Rust binaries (`matrix-bot-rust` and `matrix-mcp-server-r2`) connected in parallel to Continuwuity (`:8008`) under the same `@agent0-N` user account. Because `matrix-bot-rust` used raw `reqwest` HTTP calls without `matrix-sdk` and ignored `m.room.encrypted` events, it could not participate in E2EE rooms—and because `matrix-sdk` SQLite crypto stores and device IDs cannot be shared across two processes, running two separate E2EE clients per container would duplicate device keys, cross-signing, and `/sync` loops.

By absorbing `matrix-bot-rust`'s reactive responsibilities into a `src/bot/` module inside `matrix-mcp-server-r2`. The dispatcher stays off unless `BOT_ENABLED=true` (the default is false). The path is:
```text
<External Matrix Client> <==> <Continuwuity :8008> <==> <matrix-mcp-server-r2 (MCP + src/bot/)> <==> <Agent Zero (:80/api/api_message & :3000/mcp)>
```
1. **Single Matrix Client & Crypto Store**: One `matrix-sdk` 0.19 client, one `MATRIX_ACCESS_TOKEN`, one `.device-id`, one SQLite store, and one background `/sync` loop per agent container.
2. **Transparent Reactive E2EE**: Incoming encrypted messages (`m.room.encrypted`) are automatically decrypted by `matrix-sdk` 0.19 during the shared sync loop, filtered for `@agent0-N` triggers, and forwarded to Agent Zero's `/api/api_message`. Only `src/bot/` posts the answer, via `room.send()` (encrypted in E2EE rooms, plaintext in unencrypted rooms). The text sent to Agent Zero tells it not to use Matrix MCP tools for that turn. If decryption fails, log the event id and the reason, include no message body, and do not post in the room. History reads keep the marker `[Encrypted message - unable to decrypt (reason)]`.
3. **Operational Simplification**: The operator stops the separate `run_bot` (`matrix-bot-rust` / `matrix_bot.py`) program in `supervisord` before setting `BOT_ENABLED=true`. This process does not stop `run_bot`. While the flag is false, no invite handler and no message handler is registered.

## Core Capabilities to Absorb from `matrix-bot-rust`

1. **Trusted-Sender Invite Auto-Join**:
   - On incoming invite events (`StrippedRoomMemberEvent`), `GET {AMCTL_URL}/api/v1/trusted-senders` (default `AMCTL_URL` `http://172.23.88.254:8077`) with header `X-API-Key` from `AMCTL_API_KEY`. A non-empty `TRUSTED_SENDERS` list is the only fallback. Auto-join only when the inviter is in the trusted set. If that lookup fails and the fallback list is empty, do not join. Log the room id and the sender. No user id and no API key are compiled into the binary.
2. **Event Deduplication, Self-Suppression, and Trigger Prefix Filtering**:
   - Ignore events where `sender == client.user_id()` (self-event suppression).
   - Persist bounded processed event IDs across restarts and ignore stale messages older than 5 minutes (`300_000` ms).
   - Enforce trigger prefixes (`@agent0-N`, `agent0-N:`, `<display_name>:`, `@all-agents:`) across both encrypted and unencrypted rooms to prevent agent-to-agent infinite loops.
3. **Built-In Fast Commands (Zero-LLM)**:
   - Immediately answer `ping` (`PONG`), `hello`, and `whoami` without invoking Agent Zero.
4. **Room-to-Agent-Zero Context Mapping & `/api/api_message` Dispatch**:
   - Maintain `room_contexts.json` mapping Matrix `room_id` -> Agent Zero `context_id`.
   - Load the canonical `.a0-api-key` (reloading automatically on HTTP 401), set Matrix typing indicator (`room.typing_notice(true)`), POST the triggered message to `A0_API_URL` (`http://localhost:80/api/api_message`), chunk the reply (`<= 4000` chars), render Markdown to HTML (`pulldown-cmark`), and send via `room.send()`. Agent Zero does not post this reply.

## Success Criteria

- **SC-001**: Sending `@agent0-N: ping` and `@agent0-N: hello` in both an encrypted room and an unencrypted room (`Fleet HQ Clean`) produces an immediate reply from that agent's `matrix-mcp-server-r2` with `run_bot` stopped. `N` is the instance number in that process's Matrix user id. It is not hardcoded.
- **SC-002**: Sending a conversational prompt `@agent0-N: <question>` in an encrypted Element room decrypts via that agent's `matrix-mcp-server-r2`, routes to Agent Zero `/api/api_message`, preserves `room_contexts.json` continuity across turns, and posts one encrypted HTML/Markdown reply from `src/bot/` that is readable in Element. Agent Zero does not also post.
- **SC-003**: Inviting `@agent0-N` to a new encrypted or unencrypted room from a trusted sender (`@admin:v-site.net`) auto-joins and posts the greeting; invites from untrusted senders are ignored and logged. An invite is also ignored and logged when the trusted-sender lookup fails and `TRUSTED_SENDERS` is empty. A lab run that must name one agent uses `@agent0-5`.
