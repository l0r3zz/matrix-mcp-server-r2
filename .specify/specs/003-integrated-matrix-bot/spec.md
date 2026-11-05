# Feature Specification: Integrated Reactive Matrix Bot Dispatcher (`src/bot/`)

**Feature Branch**: `003-integrated-matrix-bot` (Stage 2 following `002-e2ee-cross-signing`)
**Created**: 2026-10-29
**Status**: Planned (Stage 2 of Unified Gateway Architecture)
**Depends On**: `.specify/specs/002-e2ee-cross-signing/spec.md` (`matrix-sdk` 0.19, persistent SQLite crypto store, cross-signing auto-bootstrap)
**Input**: Consolidate `matrix-bot-rust` directly into `matrix-mcp-server-r2` (`Option 2` from `spec/Matrix-Bot-vs-MCP-Server-Consolidation-Analysis-20261029.md`) so all inbound and outbound Matrix traffic for an Agent-Matrix triplet flows through a single process, single `matrix-sdk` 0.19 client, single `.device-id`, and single SQLite crypto store.

## Role & Motivation

In the legacy Agent-Matrix container layout, two separate Rust binaries (`matrix-bot-rust` and `matrix-mcp-server-r2`) connected in parallel to Continuwuity (`:8008`) under the same `@agent0-N` user account. Because `matrix-bot-rust` used raw `reqwest` HTTP calls without `matrix-sdk` and ignored `m.room.encrypted` events, it could not participate in E2EE rooms—and because `matrix-sdk` SQLite crypto stores and device IDs cannot be shared across two processes, running two separate E2EE clients per container would duplicate device keys, cross-signing, and `/sync` loops.

By absorbing `matrix-bot-rust`'s reactive responsibilities into a `src/bot/` module inside `matrix-mcp-server-r2` (enabled via `BOT_ENABLED=true`):
```text
<External Matrix Client> <==> <Continuwuity :8008> <==> <matrix-mcp-server-r2 (MCP + src/bot/)> <==> <Agent Zero (:80/api/api_message & :3000/mcp)>
```
1. **Single Matrix Client & Crypto Store**: One `matrix-sdk` 0.19 client, one `MATRIX_ACCESS_TOKEN`, one `.device-id`, one SQLite store, and one background `/sync` loop per agent container.
2. **Transparent Reactive E2EE**: Incoming encrypted messages (`m.room.encrypted`) are automatically decrypted by `matrix-sdk` 0.19 during the shared sync loop, filtered for `@agent0-N` triggers, forwarded to Agent Zero's `/api/api_message`, and replied to via `room.send()` (which automatically encrypts in E2EE rooms and sends plaintext in unencrypted rooms).
3. **Operational Simplification**: Retires the separate `run_bot` (`matrix-bot-rust` / `matrix_bot.py`) process in `supervisord`.

## Core Capabilities to Absorb from `matrix-bot-rust`

1. **Trusted-Sender Invite Auto-Join**:
   - On incoming invite events (`StrippedRoomMemberEvent`), query `am-ctl`'s trusted-senders endpoint (`TRUSTED_SENDERS_URL`, e.g., `http://172.23.88.254:8077/api/v1/trusted-senders` with fallback env list) and auto-join rooms only when invited by a trusted human or fleet agent.
2. **Event Deduplication, Self-Suppression, and Trigger Prefix Filtering**:
   - Ignore events where `sender == client.user_id()` (self-event suppression).
   - Persist bounded processed event IDs across restarts and ignore stale messages older than 5 minutes (`300_000` ms).
   - Enforce trigger prefixes (`@agent0-N`, `agent0-N:`, `<display_name>:`, `@all-agents:`) across both encrypted and unencrypted rooms to prevent agent-to-agent infinite loops.
3. **Built-In Fast Commands (Zero-LLM)**:
   - Immediately answer `ping` (`PONG`), `hello`, and `whoami` without invoking Agent Zero.
4. **Room-to-Agent-Zero Context Mapping & `/api/api_message` Dispatch**:
   - Maintain `room_contexts.json` mapping Matrix `room_id` -> Agent Zero `context_id`.
   - Load the canonical `.a0-api-key` (reloading automatically on HTTP 401), set Matrix typing indicator (`room.typing_notice(true)`), POST the triggered message to `A0_API_URL` (`http://localhost:80/api/api_message`), chunk the reply (`<= 4000` chars), render Markdown to HTML (`pulldown-cmark`), and send via `room.send()`.

## Success Criteria

- **SC-001**: Sending `@agent0-1: ping` and `@agent0-1: hello` in both an encrypted room and an unencrypted room (`Fleet HQ Clean`) produces an immediate reply from `matrix-mcp-server-r2` with `run_bot` stopped.
- **SC-002**: Sending a conversational prompt `@agent0-1: <question>` in an encrypted Element room decrypts via `matrix-mcp-server-r2`, routes to Agent Zero `/api/api_message`, preserves `room_contexts.json` continuity across turns, and posts an encrypted HTML/Markdown reply readable in Element.
- **SC-003**: Inviting `@agent0-1` to a new encrypted or unencrypted room from a trusted sender (`@l0r3zz:v-site.net`) auto-joins and posts the greeting; invites from untrusted senders are ignored and logged.
