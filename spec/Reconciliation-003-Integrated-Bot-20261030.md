# Reconciliation: `003-integrated-matrix-bot` artifacts vs. the approved October design

**Date**: 2026-10-30
**Mode**: Read-only. No spec, plan, task, code, branch, or dependency changes were made while producing this report. Section 7 decisions are pending owner review; no action has been taken on them.
**Target feature directory**: `.specify/specs/003-integrated-matrix-bot/` (the request named `002-e2ee-integrated-matrix-bot`, which does not exist; `003-integrated-matrix-bot` is the bot feature named by constitution Principle IX and the 002 spec).
**Baseline**: `matrix-mcp-server-r2` is an operational Rust MCP server (20 tools, `matrix-sdk` 0.10, Rust 1.88, `rmcp` 1.2). Stage 1 (`002-e2ee-cross-signing`) adds room E2EE on `matrix-sdk` 0.19. Stage 2 (`003`) absorbs the reactive `matrix-bot-rust` loop into the same process. Transport encryption (TLS via Caddy) already exists and is out of scope.
**Companion report**: `spec/Reconciliation-002-October-vs-May-20261030.md` (Stage 1 reconciliation, applied 2026-10-30).

## 1. Source inventory

| Path | Role | State | Authority |
|---|---|---|---|
| `.specify/memory/constitution.md` L13-17 (role 2), Principle IV L68-71, Principle IX L114-116 | Mandates `src/bot/` absorption and a single shared client/store | v2.0.0, committed `ba058a0` | Authoritative |
| `.specify/specs/003-integrated-matrix-bot/spec.md` | Only artifact in the feature directory: Role & Motivation, 4 capability groups, SC-001..003 | 10-29, committed `ba058a0`. No `plan.md`, `data-model.md`, `tasks.md`, `contracts/`, `research.md` | Authoritative spec, incomplete artifact set |
| `.specify/specs/002-e2ee-cross-signing/{spec,plan,data-model,tasks}.md` | Upstream dependency: shared client, store, lock, T005 sync loop, T008 trust, T010 transparent read/send, T014 gate | Reconciled 10-30 | Authoritative prerequisite |
| `spec/Matrix-Bot-vs-MCP-Server-Consolidation-Analysis-20261029.md` | Sec. 1 actual data flow; sec. 2 bot duty inventory; sec. 4-6 Architecture 1 vs 2 and phasing | 10-29 | Decision record (Architecture 2 chosen by owner) |
| `spec/E2EE-Unified-vs-TwoTier-API-Tradeoff-20261029.md` sec. 7 | Bot does not call MCP today; sec. 7.3 "delegate decrypt" is a rejected alternative | 10-29 | Decision record |
| `spec/Agent-Matrix-E2EE-Context-Synthesis-20261029.md` sec. 3-4 | Fleet layout: `/a0/usr/workdir/matrix-bot/`, shared token, identity-drift incident | 10-29 | Context |
| `spec/matrix-mcp-v1-v2.yml` L16 | Goal: single Matrix client "supporting Stage 2 absorption of matrix-bot into src/bot/" | 10-29 | Contract (defines no bot tools or config) |
| `agent-matrix-repo/multi-instance-deploy/templates/matrix-bot/rust/src/main.rs` (1017 lines), `Cargo.toml` | Current bot: raw `reqwest`, `pulldown-cmark`; `Config` L80-189; trusted senders L267-305; `handle_sync` L833-997 | Oct 17 | Baseline behavior to absorb |
| `.../templates/matrix-bot/.env.template`, `run-matrix-bot.sh`, `run-bot-wrapper.sh`, `promptinclude.md`, `matrix_bot.py`, `src/bin/set_display_name.rs` | Deployment wiring; Python runtime still selectable | Oct 17 | Baseline |
| `.../templates/scripts/supervisord-mcp-bot.conf.snippet` | `[program:run_mcp]` and `[program:run_bot]` | -- | Baseline |
| `.../am-ctl/amctl/services/{health_engine,reconciler,deployer}.py`, `routers/credentials.py:157`, `templates/scripts/fleet-token-healthcheck.sh`, `startup-services.sh` | am-ctl stops/restarts `run_bot`, syncs `matrix-bot-rust` binary and wrappers, checks `.a0-api-key`; serves `/api/v1/trusted-senders` | -- | Fleet-side dependents |
| `matrix-mcp-server-r2/src/matrix/client.rs:60-74`, `src/main.rs`, `src/config.rs` | Existing background `sync_once` loop; no `bot` module; no `BOT_ENABLED` | Mar 2026 | Baseline |
| `.specify/feature.json` | Points at `002-e2ee-cross-signing` | gitignored | Wiring |
| Git | Branches: `002-e2ee-cross-signing`, `main`. No `003-integrated-matrix-bot` branch despite the spec header | -- | Wiring |

## 2. Decision provenance

| ID | Decision / required behavior | Class | Evidence |
|---|---|---|---|
| B1 | Absorb `matrix-bot-rust` into `src/bot/` as one binary (Architecture 2); retire `run_bot` | Approved (owner chose Option 2, 10-29) | Constitution IX L114-116; 003 spec Input/Role; Consolidation sec. 4 Architecture 2, sec. 6 Phase B |
| B2 | One `matrix-sdk` client, one token, one `.device-id`, one SQLite store, one `/sync` loop shared by MCP and bot | Approved | Constitution IV L68-71; 003 spec Role item 1; 002 spec Edge Case "two processes" |
| B3 | 003 starts only after the 002 T014 gate | Approved | 002 `tasks.md` Dependencies; constitution IX |
| B4 | Bot gated by `BOT_ENABLED=true` | Proposal carried into spec (not in constitution) | 003 spec Role; Consolidation sec. 4 Architecture 2 |
| B5 | Trusted-sender auto-join via am-ctl, fallback list, retries, greeting | Approved behavior to preserve; env var name unresolved | 003 spec Capability 1; `main.rs:267-305, 833-875` |
| B6 | Self-suppression, bounded persistent dedupe (1000 ids), drop events older than 5 min | Approved behavior to preserve | 003 spec Capability 2; `main.rs:307-321, 902-929` |
| B7 | Trigger prefixes required even in 1:1 rooms (loop prevention) | Approved behavior to preserve | 003 spec Capability 2; `main.rs:151-168, 946-969` |
| B8 | Fast commands `ping` / `hello` / `whoami` without LLM | Approved behavior to preserve | 003 spec Capability 3; `main.rs:629-637` |
| B9 | `room_contexts.json`, `.a0-api-key` with 401 reload, typing indicator, Markdown to HTML, chunked reply via `room.send()` | Approved behavior to preserve; chunk size conflicts | 003 spec Capability 4; `main.rs:417-553, 981-994` |
| B10 | Encrypted rooms: decrypt in shared sync, reply encrypts automatically, cross-signed trust unchanged | Approved (inherits 002 D4/D12) | 003 spec Role item 2; constitution V, VI |
| B11 | Architecture 1 (bot as local client of MCP over `:3000`) and sec. 7.3 "delegate decrypt" | Rejected alternatives | Consolidation sec. 4 Architecture 1, sec. 5; Tradeoff sec. 7.3 |
| B12 | Python bot (`matrix_bot.py`) retired | Historical / implied | agent-matrix `AGENTS.md` "Rust (primary), Python (legacy)"; `run-matrix-bot.sh` still offers `python` mode |
| B13 | System-prompt wrapper injected before Agent Zero ("DO NOT use any Matrix tools, do not send messages via MCP") | Unresolved: existing behavior not mentioned in 003 spec | `main.rs:982-984`; Consolidation sec. 1.2 explains why it exists (duplicate replies) |
| B14 | Display name set at startup; `set-display-name-rust` helper binary | Unresolved: existing behavior not in spec | `main.rs:323-341`; `src/bin/set_display_name.rs` |
| B15 | Where fleet-side retirement work lives (supervisord, am-ctl health/reconciler/deployer, `.env`, binaries) | Unresolved: cross-repo | 003 spec Role item 3 (one sentence); am-ctl files in the inventory |

## 3. Decision-coverage matrix

Status key: covered / partial / missing / conflicting / deferred.

| Decision | Spec requirement | Plan / data-model / contracts | Task | Acceptance / verification | Status |
|---|---|---|---|---|---|
| B1 absorb into `src/bot/` | Role; Input | none (no `plan.md`) | none (no `tasks.md`) | SC-001 "with `run_bot` stopped" | partial -- spec only |
| B2 single client/store | Role item 1 | 002 data-model lock row covers refusal; no 003 design for handler registration on the shared `Client` | none | none | partial |
| B3 sequencing after 002 | Header "Depends On" | 002 tasks Dependencies | none | 002 T014 | covered |
| B4 `BOT_ENABLED` gate | Role | none; `src/config.rs` has no field | none | none | partial -- behavior with `SKIP_MATRIX_INIT=true` or E2EE off is unspecified |
| B5 trusted auto-join | Capability 1 | none | none | SC-003 | conflicting -- spec says `TRUSTED_SENDERS_URL` "with fallback env list"; code uses `AMCTL_URL` + `AMCTL_API_KEY` (`main.rs:140-143`) and a hardcoded fallback (`@admin:v-site.net`, `@l0r3zz:v-site.net`, L300-304); spec omits refresh cadence (every 60 syncs and on invite), 4-attempt join retry, greeting text |
| B6 dedupe / self / stale | Capability 2 | none -- where `processed_events` persists once `sync_cursor.json` is replaced by the SDK state store is undefined | none | none | partial |
| B7 trigger prefixes | Capability 2 | none | none | SC-001/002 implicitly | partial -- spec lists `@agent0-N`, `agent0-N:`, `<display_name>:`, `@all-agents:`; code also accepts comma variants and `@<display_name>` (L157-168) and strips mention pills (L49, L941); `TRIGGER_PREFIX` in `.env.template` is unused by the Rust bot |
| B8 fast commands | Capability 3 | none | none | SC-001 (`ping`, `hello`) | partial -- `whoami` has no SC |
| B9 Agent Zero dispatch | Capability 4 | none | none | SC-002 (context continuity, HTML reply) | conflicting -- spec says chunk `<= 4000` chars; code `MAX_CHARS_PER_MESSAGE = 32000` (L14); spec omits the 404 context-reset path (L482-494) and the 300 s Agent Zero timeout |
| B10 encrypted reactive path | Role item 2 | relies on 002 T005/T008/T010 | none | SC-002 | partial -- UTD trigger message handling undefined; no SC for the cross-signed rule on bot replies |
| B11 rejected alternatives | -- | -- | -- | -- | covered (documented as rejected) |
| B12 Python bot retirement | silent | -- | none | none | missing |
| B13 system-prompt wrapper | silent | -- | none | none | missing / owner decision |
| B14 display name | silent | -- | none | none | missing |
| B15 fleet-side retirement | Role item 3 only | -- | none | SC-001 assumes `run_bot` stopped | missing -- no owner, no tasks, cross-repo |

## 4. Findings

### 4.1 May assumptions superseded

None specific to 003 (it was created in October). The design-era assumption that `matrix-bot` is a separate Python process (`agent-matrix-repo/multi-instance-deploy/docs/agent-matrix-design.md:187, 394, 420`) is superseded by B1; those docs still describe it that way.

### 4.2 Missing requirements

The spec has no FR/NFR section (Role, Capabilities, SC only). Nothing specifies:

- `BOT_ENABLED` interaction with `SKIP_MATRIX_INIT` and with E2EE disabled.
- Persistence location for `room_contexts.json` and dedupe state once the bot runs under the MCP working directory.
- UTD trigger message handling (ignore, or reply "cannot decrypt").
- Display-name behavior at startup.
- System-prompt wrapper (keep, drop, configurable).
- `.a0-api-key` path: am-ctl `reconciler.py:96` checks `/a0/usr/workdir/matrix-bot/.a0-api-key`; the MCP process lives in `/a0/usr/workdir/matrix-mcp-server/`.
- Fleet-side retirement scope and owner.

### 4.3 Obsolete tasks

None (no tasks exist).

### 4.4 Contradictions

- Spec vs code: chunk size (4000 vs 32000); trusted-sender env var (`TRUSTED_SENDERS_URL` vs `AMCTL_URL`); fallback source ("env list" vs hardcoded pair).
- Spec header "Feature Branch: `003-integrated-matrix-bot`" vs no such git branch.
- 002 spec Edge Case text and Tradeoff sec. 7.3 cite "`spec.md:85`" for the two-process rule; after the 10-30 edits it sits at `spec.md:103` (low).

### 4.5 Already implemented, do not repeat

- `src/matrix/client.rs:60-74` already runs the background sync loop the bot handler would attach to.
- 002 T005/T008/T010 deliver decrypt/encrypt and the cross-signed recipient rule.
- `send-message` (`src/mcp/server.rs`) already sends via `room.send()`; `get-room-info` already exposes `Encrypted`.
- Nothing in `src/` implements bot behavior yet.

### 4.6 Tasks marked complete without evidence

None (no tasks).

### 4.7 Decisions lacking a testable acceptance criterion

B4, B5 (retry, fallback, untrusted-invite logging), B6, B7 (loop prevention in 1:1 rooms), B8 `whoami`, B10 UTD, B13, B14, B15.

## 5. Workflow wiring

- `.specify/feature.json` points at `002-e2ee-cross-signing`. `/speckit-implement` would run 002's tasks, not 003. To work on 003 you must run `create-new-feature.sh` (creates branch + directory + rewrites `feature.json`), edit `feature.json`, or export `SPECIFY_FEATURE_DIRECTORY`. Spec Kit v1.2.3 holds one active feature at a time; switching away from 002 mid-implementation and back is manual.
- Even with the pointer switched, `check-prerequisites.sh --require-tasks` fails for 003: `plan.md not found` (`check-prerequisites.sh:145-149`). Required order: `/speckit-plan` (`setup-plan.sh` copies the template into the feature directory) -> `/speckit-tasks` (`setup-tasks.sh`, writes `tasks.md`) -> `/speckit-analyze` -> `/speckit-implement`.
- File name expected: `tasks.md` (`common.sh:224`). No `task.md` exists anywhere in the repo.
- Auto-read by `/speckit-implement` once artifacts exist: `tasks.md`, `plan.md`, `data-model.md`, `contracts/`, `research.md`, `quickstart.md`, `checklists/`, and `.specify/memory/constitution.md`. Not read: 003 `spec.md` itself, the Consolidation and Tradeoff analyses, `main.rs` in `agent-matrix-repo`, am-ctl sources. Behavior to preserve from `main.rs` must be transcribed into plan/data-model/tasks or it will not reach the implementer.
- `setup-plan.sh` creates the feature directory if missing; running it with a wrong `feature.json` silently creates a stray directory with a blank `plan.md`.

## 6. Prioritized discrepancies

1. Blocker: 003 has no plan, data-model, or tasks; `feature.json` points at 002; no 003 branch. Nothing is implementable yet.
2. High: fleet-side retirement (supervisord `run_bot`, am-ctl health/reconciler/deployer, `.a0-api-key` path, `fleet-token-healthcheck.sh`, `startup-services.sh`) has no owner, spec, or tasks; SC-001 assumes it is done.
3. High: spec/code conflicts: chunk size, trusted-sender env var and fallback, missing comma and `@<display_name>` prefixes, 404 context reset, mention-pill stripping.
4. Medium: persistence redesign undefined (`room_contexts.json`, dedupe state, `.a0-api-key` location) once the bot runs inside `/a0/usr/workdir/matrix-mcp-server/`.
5. Medium: UTD trigger handling and `BOT_ENABLED` x `SKIP_MATRIX_INIT` / E2EE-off behavior unspecified.
6. Medium: spec has no FR/NFR section, so `/speckit-analyze` coverage mapping will have no requirement keys to map tasks to.
7. Low: system-prompt wrapper and display-name behavior silently dropped; Python bot retirement implied, not stated; stale `spec.md:85` cites; `agent-matrix-design.md` still describes the Python bot.

## 7. Decisions I need from you (pending; no action taken)

1. Fleet-side scope. Does 003 own the `agent-matrix-repo` changes (supervisord snippet, am-ctl health/reconciler/deployer, wrappers, `.env`), or does 003 end at "binary supports `BOT_ENABLED`" with a separate agent-matrix runbook or feature for retirement?
2. System-prompt wrapper. Keep injecting the "[MATRIX MESSAGE ... DO NOT use any Matrix tools, do not send messages via MCP]" preamble now that bot and MCP are one process (it still prevents double replies), drop it, or make it configurable?
3. Conflicts to resolve in the spec: chunk size -- 32000 (code) or 4000 (spec)? Trusted senders -- keep `AMCTL_URL` / `AMCTL_API_KEY` names and the hardcoded fallback pair, or move to `TRUSTED_SENDERS_URL` plus an env fallback list?
4. State location. Keep `room_contexts.json` and `.a0-api-key` at `/a0/usr/workdir/matrix-bot/` (am-ctl keeps working unchanged), or move them under `/a0/usr/workdir/matrix-mcp-server/` with am-ctl updated?
5. Spec Kit mechanics. Create the 003 branch and run `/speckit-plan` and `/speckit-tasks` now (switching `feature.json` to 003), or hold 003 scaffolding until 002 T014 passes and only patch `spec.md` now?
6. Secondary (can default): record Python bot retirement and display-name behavior in the spec; add FR/NFR section so `/speckit-analyze` has requirement keys.

## 8. Proposed patch set (not applied; awaiting review of sections 1-7)

Listed so the review can accept or strike items. Nothing below has been executed.

- `003/spec.md`: add Requirements (FR-001..) and NFR sections; add `BOT_ENABLED` x `SKIP_MATRIX_INIT` / E2EE-off edge cases; add UTD trigger behavior; fix chunk size and trusted-sender env naming per decision 3; add comma and `@<display_name>` prefixes, mention-pill stripping, 404 context reset, join retry and greeting, refresh cadence; add display name and system-prompt decisions per decision 2; state Python bot retirement; add SC for `whoami`, loop prevention in 1:1 rooms, untrusted-invite rejection logging; correct the branch header or create the branch.
- `003/plan.md`, `003/data-model.md`, `003/tasks.md`: generate via `/speckit-plan` and `/speckit-tasks` after `feature.json` is switched (decision 5), transcribing the `main.rs` behavior inventory so `/speckit-implement` sees it.
- Fleet-side: either a `tasks.md` phase in 003 or a separate `agent-matrix-repo` runbook (decision 1).
- Low-priority cite fixes: `002/spec.md` Edge Case and Tradeoff sec. 7.3 "`spec.md:85`" -> current line; `agent-matrix-design.md` Python-bot wording.
