# Reconciliation: `002-e2ee-cross-signing` artifacts vs. the approved October design

**Date**: 2026-10-30
**Mode**: Read-only. No spec, plan, task, code, branch, or dependency changes were made while producing this report.
**Target feature directory (on disk at time of review)**: `.specify/specs/002-e2ee-cross-signing - Oct/`
**Baseline**: `matrix-mcp-server-r2` is an operational Rust MCP server (20 tools, `matrix-sdk` 0.10, Rust 1.88, `rmcp` 1.2). The enhancement adds room-level E2EE on `matrix-sdk` 0.19 while preserving existing functionality. Transport encryption (TLS via Caddy) already exists and is out of scope.

## 1. Source inventory

| Path | Role | Date / state | Authority |
|---|---|---|---|
| `.specify/memory/constitution.md` | Governance v2.0.0, Principles I-X, locked stack | Amended 2026-10-29, committed `ba058a0` | Authoritative |
| `.specify/specs/002-e2ee-cross-signing - Oct/spec.md` | October feature spec (US1-5, FR-001..010, NFR-001/002, SC-001..005) | Created 10-23, updated 10-29; uncommitted U1-U3 edits; directory renamed on disk, untracked | Authoritative (target) |
| `... - Oct/plan.md` | Technical context, constitution check, source map | 10-29; untracked path | Authoritative |
| `... - Oct/data-model.md` | On-disk layout, `EncryptionMode`, `BootstrapStatus`, error taxonomy | 10-29; uncommitted U2 edit | Authoritative |
| `... - Oct/tasks.md` | T001-T014, 5 phases, dependency line | 10-29; all 14 unchecked | Authoritative |
| `.specify/specs/003-integrated-matrix-bot/spec.md` | Stage 2 bot absorption; spec only (no plan/tasks) | 10-29 | Authoritative, out of 002 scope |
| `.specify/specs/001-e2ee-support - May/spec.md` | May spec: `-r2` split, `EncryptedRoomError`, 0.10, Dendrite, optional cross-signing | 2026-05-11; header marked Superseded; renamed, untracked | Historical |
| `spec/matrix-mcp-v1-v2.yml` | Unified 20+2 tool contract v2.0.0 | 10-29 | Authoritative contract |
| `spec/E2EE-Key-Management-Model.md` | Section 0 locked decisions (10-29) plus older body (0.1-9) | Mixed | Section 0 authoritative; body stale |
| `spec/E2EE-Landscape-Review-October-20261023.md` | October briefing, locked-decision table | 10-23; 3 lines updated 10-29 | Authoritative briefing |
| `spec/E2EE-Landscape-Review-20261023.md` | May-era review with 10-23 status banner | Historical | Historical |
| `spec/E2EE-Prep-Checklist-for-Rust-MCP-Server.md` | Pre-design checklist; `-r2` design; TOFU boxes checked | May | Historical, unmarked |
| `spec/E2EE-Unified-vs-TwoTier-API-Tradeoff-20261029.md` sec. 8 | Owner decisions locked 10-29 (unified, auto fallback, auto-bootstrap, no TOFU) | 10-29 | Decision record |
| `spec/Matrix-Bot-vs-MCP-Server-Consolidation-Analysis-20261029.md` sec. 6 | Option 2 / two-stage decision | 10-29 | Decision record |
| `spec/Speckit-Analyze-Report-20261029.md` | Run 2, zero findings, "Ready for /speckit-implement" | Uncommitted | Derived |
| `Cargo.toml`, `Dockerfile`, `.github/workflows/docker-publish.yml` | Compiling pin 0.10 / rust 1.88; `e2ee` feature = `matrix-sdk/e2e-encryption` only | Code | Baseline |
| `src/config.rs`, `src/main.rs`, `src/matrix/client.rs`, `src/mcp/server.rs`, `src/error.rs` | Operational server: 20 tools, `MCP_SERVER` device id, `/health` without e2ee fields, `SKIP_MATRIX_INIT` mode | Code (Mar 2026) | Baseline |
| `tests/{integration,config,cache,server_tool_list}_tests.rs` | wiremock tests; `test_config` builds `Config` by struct literal | Code | Baseline |
| `.specify/feature.json` | `{"feature_directory":".specify/specs/002-e2ee-cross-signing"}` | gitignored; points to a path that no longer exists | Wiring |
| `.specify/scripts/bash/{common.sh,check-prerequisites.sh,setup-plan.sh,setup-tasks.sh}`, `.cursor/skills/speckit-implement/SKILL.md` | Spec Kit v1.2.3 resolution + implement command | Installed | Wiring |

## 2. Decision provenance (October design)

| ID | Decision | Primary evidence |
|---|---|---|
| D1 | Unified 20-tool surface; no `-r2` duplicates; no `EncryptedRoomError` refusals; plus `bootstrap-e2ee`, `get-encryption-status` | Tradeoff sec. 8; constitution I; yml `non_goals` L19 |
| D2 | `encryptionMode` `auto|e2ee|none`; `auto` pre-flight `/keys/query`; fall back on HTTP 200 "no keys"; fail closed on network/federation error | Tradeoff sec. 4/8; constitution I L34-40; yml L13, L319 |
| D3 | Auto-bootstrap at startup when `E2EE_ENABLED=true`; tool remains callable | Tradeoff sec. 8; constitution II; Key-Mgmt sec. 0 |
| D4 | Cross-signed only; TOFU / `ignore_unverified_devices` forbidden; withhold `m.unverified` | Oct landscape "Locked decisions"; constitution V; Key-Mgmt sec. 0 |
| D5 | `matrix-sdk` 0.19.x (`e2e-encryption`, `bundled-sqlite`, rustls), Rust >= 1.96, crypto >= 0.16.1, `rmcp` stays 1.2 | Constitution Locked Stack; Oct landscape; Cargo.toml comments |
| D6 | One device id per agent in `$CRYPTO_STORE_PATH/.device-id`, aligned with `/whoami`; `MCP_SERVER` literal banned | Oct landscape Finding 8; constitution VIII; Key-Mgmt sec. 0 |
| D7 | Persistent SQLite store at `CRYPTO_STORE_PATH` (0700), exclusive process lock, single store owner | Constitution IV, VIII; Consolidation analysis sec. 3 |
| D8 | Recovery secret precedence ref -> env-var name -> `MATRIX_RECOVERY_KEY_FILE`; never raw secret in args/logs/results; `backupPassphraseRef` -> `SecretResolutionError` until a backend exists | Key-Mgmt sec. 0/0.1; data-model; U2 remediation |
| D9 | Never reset an existing cross-signing identity without a secret (`IdentityExistsError`); `forceRecreate` rotates local store only | Oct landscape "Identity reset"; U2 remediation |
| D10 | Cold bootstrap <= 60 s; key-backup restore async with `keyBackupRestore` state | U1 remediation; NFR-001 |
| D11 | `/health` always has `e2ee` / `crossSigned` booleans; non-`e2ee` builds return stub message on the 2 tools | U3/I1 remediation; constitution II |
| D12 | Transparent decrypt on reads; UTD events rendered `[Encrypted message - unable to decrypt (reason)]` | Constitution VI; qechris prior art |
| D13 | Continuwuity encrypted group room is gate 1; DM is gate 2; DM failure never relaxes trust | Oct landscape Homeserver row; spec Edge Cases |
| D14 | Stage 2: absorb `matrix-bot-rust` into `src/bot/` (003) after 002 | Consolidation sec. 6; constitution IX |
| D15 | Preserve existing functionality; existing tests pass on both feature builds | Owner baseline statement; SC-005 |

## 3. Decision-coverage matrix

Status key: covered / partial / missing / conflicting / deferred.

| Decision | Spec requirement | Plan / data-model / contract | Task | Acceptance / verification | Status |
|---|---|---|---|---|---|
| D1 unified surface | Role; FR-006, FR-007 | plan Summary; yml `unified` block | T009, T010, T011 | SC-002, SC-004; T014 | covered |
| D2 auto pre-flight + fail closed | US3 AC1-4; FR-006 | data-model `EncryptionMode`; yml L134-145, L319 | T009 | SC-004; T013 (fallback vs encrypt); T014 | partial -- fail-closed on network error has no test item |
| D3 startup auto-bootstrap | US2; FR-004 | plan Summary | T007 | US2 Independent Test (manual) | partial -- no automated test; `SKIP_MATRIX_INIT` interaction unspecified |
| D4 cross-signed only | US4 AC2; FR-005 | yml L21/L28; data-model `TrustPolicy` | T008 | SC-003; T014 | covered |
| D5 SDK 0.19 / rust 1.96 / rmcp 1.2 | SDK table; FR-001, FR-010 | plan Technical Context | T001, T002 | SC-005; T012, T013 | covered (Cargo `e2ee` feature currently lacks `bundled-sqlite`; T001 adds it) |
| D6 `.device-id` + `/whoami` | US1 AC1-3; FR-003 | data-model On-Disk Layout | T002, T004, T005 | SC-001; T013 | covered |
| D7 store + lock + single owner | US1; FR-002; Edge Case "two processes" | data-model lock row; `KeyStoreUnavailableError` | T003, T004 | T013 "store lock contention" | covered |
| D8 secret precedence / redaction | US2 AC1; FR-008 | data-model `RecoverySecret`; Key-Mgmt sec. 0.1 | T006 | none | partial -- no test for precedence, `SecretResolutionError`, or redaction |
| D9 `IdentityExistsError`, `forceRecreate` | US2 AC3; FR-004 | data-model Errors | T007 | US2 Independent Test (manual) only | partial -- not in T013 or T014 |
| D10 60 s + async `keyBackupRestore` | NFR-001; US5 | data-model `BootstrapStatus`; yml L266/L310 | T007, T011 | none measurable | partial |
| D11 `/health` + stubs | US5; FR-001, FR-009 | plan `src/main.rs` row | T011 | T012 | covered |
| D12 UTD marker | US4 AC3; FR-007 | data-model `MessageDecryption` | T010 | T013 (marker not named); T014 omits UTD | partial |
| D13 group-room gate 1, DM gate 2 | Edge Cases; SC-002 | plan Testing row | T014 (group room, restart, unverified device, fallback) | DM test absent from T014 | partial -- `send-direct-message` E2EE path has no acceptance gate |
| D14 Stage 2 bot | Followed By; constitution IX | 003 spec exists; no plan/tasks | -- | 003 SC-001..003 | deferred (intentional) |
| D15 preserve existing behavior | SC-005; FR-001 | plan Constraints | T002, T012 | existing wiremock suite | covered; `tests/integration_tests.rs:29-56` `Config` struct literal will break when T003 adds fields |
| `trustPolicy` param -> `TrustPolicyNotSatisfiedError` | US2 AC4 | yml; data-model Errors | implied by T011 | none | partial -- contract-only |
| UIAA password for `/keys/device_signing/upload` | US2 AC2 | plan `src/config.rs` row | T003, T007 | Continuwuity manual (T014 does not name it) | partial |

## 4. Findings

### 4.1 May assumptions superseded (correctly, in the October set)

`-r2` tool split and `EncryptedRoomError` refusal (May US5, FR-008/009/010, SC-004); `encryption_mode` `none|e2ee` defaulting to `none` (Oct: `auto|e2ee|none`, default `auto`); optional cross-signing (Oct: mandatory); `matrix-sdk` 0.10 (Oct: 0.19, CVE-2026-45056 floor); Dendrite (Oct: Continuwuity); 30 s bootstrap (Oct: 60 s with async backup); "corrupt store -> re-bootstrap new keys" (Oct: refuse / `IdentityExistsError`; align `.device-id` before upload); `forceRecreate` clears store and mints a new identity (Oct: local rotation only).

### 4.2 Missing requirements / obsolete tasks

- No obsolete tasks: T001-T014 all map to October decisions.
- Missing verification tasks for D2 fail-closed, D8, D9, D10, D12 marker, D13 DM gate.
- Spec is silent on `SKIP_MATRIX_INIT` (`src/config.rs:36`, `src/main.rs:36-46`): what `/health` and the two management tools return in that mode, and that auto-bootstrap must not run.
- No `contracts/` directory in the feature folder, so `spec/matrix-mcp-v1-v2.yml` is not auto-loaded by `/speckit-implement`.

### 4.3 Contradictions between documents

1. Path rename: roughly 55 cross-references in about 20 files (constitution L20-21/L192, plan L3, data-model L4, tasks L3, 003 spec L6, 001 spec L5, `AGENTS.md`, `src/AGENTS.md`, `.cursor/rules/*.mdc`, `Cargo.toml` comments, nine `spec/*.md` reports) point at `.specify/specs/002-e2ee-cross-signing/`, which no longer exists on disk. Same for `001-e2ee-support/`.
2. `spec/E2EE-Key-Management-Model.md` sec. 0 says unified / no `-r2`, but sec. 0.1 (L35), 2 (L92), 4 (L129, L142-143, L153), 5 (L167), 7 (L190-216), 8 (L230 `EncryptedRoomError` gate) still describe the `-r2` two-tier design and `encryption_mode=e2ee`.
3. `spec/E2EE-Prep-Checklist-for-Rust-MCP-Server.md` has TOFU checked (L230-231), `-r2` design throughout, `EncryptedRoomError`; no superseded banner.
4. Constitution VIII says `/health` reflects e2ee status "when E2EE is enabled"; spec FR-009 says "always". Compatible (spec stricter) but wording differs.
5. `Speckit-Analyze-Report` (uncommitted) says "Ready for `/speckit-implement`" while the implement prerequisite check currently fails.

### 4.4 Already-implemented work the spec/tasks should acknowledge

- `Config.e2ee_enabled` and `crypto_store_path` already exist (`src/config.rs:29-30, 111-114`); T003 extends rather than creates.
- `e2ee` Cargo feature exists (maps only to `matrix-sdk/e2e-encryption`).
- `get-room-info` already reports `Encrypted: Yes/No` via `room.is_encrypted()` (`src/mcp/server.rs:278-282`).
- `create-room` already supports `inviteUsers` / `isPrivate` (`server.rs:564-592`); T009 extends it.
- `parse_message_events` filters `m.room.message` only (`server.rs:238`); T010 must also accept decrypted `m.room.encrypted` output.

### 4.5 Tasks marked complete without evidence

None. All 14 tasks are unchecked and no code matches any task.

### 4.6 Decisions lacking a testable acceptance criterion

D2 fail-closed, D8 redaction/precedence, D9 `IdentityExistsError`, D10 60 s / `keyBackupRestore`, D13 DM, `trustPolicy` rejection, UIAA path.

## 5. Workflow wiring

- Resolution order (`common.sh:184-209`): `SPECIFY_FEATURE_DIRECTORY` env var -> `.specify/feature.json` -> error. The git branch name is not used; the directory rename, not the branch, broke resolution.
- Verified: `check-prerequisites.sh --json --require-tasks --include-tasks` (the exact call `/speckit-implement` makes) exits 1 with `Feature directory not found: .../.specify/specs/002-e2ee-cross-signing`. `/speckit-implement`, `/speckit-analyze`, `/speckit-tasks`, `/speckit-plan` are blocked until `feature.json` (or the env var) points at an existing directory.
- File name: scripts emit `TASKS=$feature_dir/tasks.md` (`common.sh:224`) and the implement skill reads `tasks.md`; `task.md` is never read. No `task.md` exists in the repo.
- `/speckit-implement` auto-reads: `tasks.md` (required), `plan.md` (required), `data-model.md`, `contracts/`, `research.md`, `quickstart.md`, `checklists/`, and `.specify/memory/constitution.md`. It does not read `spec.md` directly, nor `spec/matrix-mcp-v1-v2.yml`, `E2EE-Key-Management-Model.md`, the landscape reviews, or the tradeoff/consolidation analyses.
- Write side effects of `/speckit-implement`: marks `[X]` in `tasks.md`; "create/verify" `.gitignore` and `.dockerignore` (both exist; patterns may be appended). No `extensions.yml` hooks present.
- `setup-plan.sh` (used by `/speckit-plan`) copies the plan template into the resolved feature directory, creating the directory if needed. Running it with a stale `feature.json` silently recreates the old path with a blank `plan.md`.
- Spaces in `... - Oct` are quoted safely by `printf %q`, but skills paste the path into shell commands, raising the chance of a mis-quoted command during implement.

## 6. Prioritized discrepancies

1. Blocker: `feature.json` points at a non-existent path; about 55 doc cross-references stale; git shows originals deleted and renamed directories untracked.
2. High: six October decisions have no test/verification task (D2 fail-closed, D8, D9, D10, D12 marker, D13 DM gate).
3. High: `send-direct-message` E2EE path (US3/FR-006) has no acceptance gate because T014 omits the DM test the spec names.
4. Medium: `SKIP_MATRIX_INIT` x E2EE behavior unspecified.
5. Medium: Key-Management-Model body and Prep Checklist still describe `-r2` / TOFU / `EncryptedRoomError`.
6. Low: T003 will break `tests/integration_tests.rs` `test_config`; T003/T012 do not mention it. T010 must handle decrypted `m.room.encrypted` events in `parse_message_events`.
7. Low: constitution VIII vs FR-009 wording; `Speckit-Analyze-Report` "ready" claim predates the rename; U1-U3 edits and the report are uncommitted.

## 8. Resolution applied (2026-10-30, after owner approval)

| Decision | Owner choice | Change |
|---|---|---|
| Directory names | Revert | `... - Oct` -> `.specify/specs/002-e2ee-cross-signing`, `... - May` -> `.specify/specs/001-e2ee-support`. `feature.json` unchanged and resolving; `check-prerequisites.sh --require-spec --require-tasks` passes. |
| Test gaps | Add before implement | `spec.md`: new `SC-006`; `tasks.md`: T003 (fixture note), T007 (`trustPolicy` rejection, `SKIP_MATRIX_INIT`), T010 (`parse_message_events` must include decrypted/UTD events), T012 (`/health` under `SKIP_MATRIX_INIT`), T013 expanded to itemized cases (fail-closed, secret precedence/redaction, `IdentityExistsError`, `keyBackupRestore`, `trustPolicy`, UTD marker), T014 gains DM gate and 60 s / UIAA cold-bootstrap check. Task count stays 14. |
| Stale documents | Rewrite | `E2EE-Key-Management-Model.md` body (0.1, 2, 4, 5, 7, 8) now uses unified tool names and auto-fallback semantics; `keyBackupRestore` added to 7.1/7.2. `E2EE-Prep-Checklist` gained a superseded banner; the TOFU decision boxes now record the cross-signed decision. |
| `SKIP_MATRIX_INIT` | Specify | `spec.md` Edge Cases, FR-004, FR-009; `data-model.md` `CryptoStoreState`; tasks T007, T011, T012. |

Not changed (not in scope of the approved choices): constitution VIII wording ("when E2EE is enabled" vs FR-009 "always") remains a future amendment item.

Follow-up 2026-10-31: constitution amended to v2.0.1 (Security First hostile-network paragraph; E2EE target MSRV 1.96 -> 1.99, propagated to spec/plan/tasks, `Cargo.toml` comments, rules, and the October landscape review). `/speckit-analyze` Run 3 (`spec/Speckit-Analyze-Report-20261031.md`) found 0 critical/high; its two actionable findings were applied: T015 adds the CI matrix required by Principle VII, and `MATRIX_UIAA_PASSWORD_FILE` is now named in `data-model.md`, T003, and T014. The coverage matrix above is otherwise unchanged; the "UIAA password" row moves from partial to covered.

## 7. Owner decisions requested (as posed before approval)

1. Directory names: revert to `002-e2ee-cross-signing` / `001-e2ee-support` (recommended), or keep ` - Oct` / ` - May` and update `feature.json` plus every cross-reference.
2. Test gaps: add tasks (or extend T013/T014) for the six untested decisions before implementing, or record them as deferred in `tasks.md`.
3. Stale documents (`E2EE-Key-Management-Model.md` sec. 0.1-9, Prep Checklist): rewrite to unified terms, add a historical banner, or delete.
4. `SKIP_MATRIX_INIT`: specify that E2EE is disabled and `/health` reports `e2ee:false, crossSigned:false` in that mode (suggested), or leave out of scope.
