# Specification Analysis Report (`/speckit-analyze` -- Run 5)

**Date**: 2026-11-01
**Feature Directory**: `.specify/specs/002-e2ee-cross-signing`
**Inputs**: `.specify/memory/constitution.md` (v2.0.2), `spec.md` (working tree, Updated 2026-11-01), `plan.md`, `data-model.md`, `tasks.md` (T001-T015), `contracts/README.md`, `spec/matrix-mcp-v1-v2.yml`
**Prerequisite check**: `check-prerequisites.sh --json --require-spec --require-tasks --include-tasks` -> `AVAILABLE_DOCS: ["data-model.md","contracts/","tasks.md"]`. No `.specify/extensions.yml`.
**Branch check**: `main` is an ancestor of `002-e2ee-cross-signing` (merge-base `ccb2c0e`). The Role sentence "forked from `main`" is true.
**Trigger**: Pre-implementation check after the Role clarification (in-place upgrade of the existing crate). Supersedes `spec/Speckit-Analyze-Report-20261031-Run4.md`.

This run does not modify `spec.md`, `plan.md`, `tasks.md`, `data-model.md`, or the constitution.

---

## 1. Findings

No findings.

The Role sentence added on 2026-11-01 restates constraints the plan and tasks already carry. `plan.md` says "Upgrade the existing 20 MCP tools in place." `tasks.md` says "T001 is the first code change" and names the current files (`src/matrix/client.rs`, `src/mcp/server.rs`, `src/config.rs`, `tests/integration_tests.rs`). FR-001 and SC-005 already require the non-`e2ee` build and the existing tests to keep passing. Constitution Principle I keeps the 20 tool names. No new requirement was added, so no task, data-model, or contract edit is required.

Run 4 findings I1, I2, U2, and I3 stay closed: constitution v2.0.2 matches FR-009, T015 blocks recording T014 complete, clippy and test run per feature build, and `spec/matrix-mcp-v1-v2.yml` names `MATRIX_UIAA_PASSWORD_FILE` and `UiaaRequiredError`.

Duplication: none. Unresolved placeholders: none. Vague adjectives without a measurable criterion: none.

`Cargo.toml` still has `default = []`, so SC-005's `--no-default-features` and T012/T015's plain `cargo test` are the same build. Noted so a future default feature does not desynchronize them.

---

## 2. Coverage Summary

| Requirement Key | Summary | Has Task? | Task IDs | Notes |
|-----------------|---------|-----------|----------|-------|
| Role in-place upgrade (`spec.md:12`) | Upgrade the crate on this branch; T001 is the first code change; 20 tools, wiremock tests, and the non-`e2ee` build stay | Yes | T001, T002, T003, T010, T012 | Unnumbered constraint. Matches plan Summary |
| US1 | Per-agent `.device-id` via `/whoami`, isolated store | Yes | T002, T003, T004, T005, T013 | |
| US2 | Startup auto-bootstrap, recovery secret, UIAA, `IdentityExistsError`, `UiaaRequiredError`, `trustPolicy` | Yes | T003, T006, T007, T011, T013, T014(6) | AC5 covered by T007 and T013 |
| US3 | `encryptionMode` auto / e2ee / none with `/keys/query` pre-flight | Yes | T009, T013, T014(4) | |
| US4 | Transparent send/read, cross-signed-only key share, UTD marker | Yes | T008, T010, T013, T014(1-3) | |
| US5 | `get-encryption-status`, `/health`, non-`e2ee` stubs, `SKIP_MATRIX_INIT` | Yes | T011, T012, T013 | |
| Edge: wiped store | Align `.device-id` before `/keys/upload` | Yes | T004, T007 | |
| Edge: two processes | `KeyStoreUnavailableError` lock | Yes | T004, T013 | |
| Edge: encrypted DMs | DM gate recorded as a homeserver result | Yes | T014(5) | |
| Edge: `SKIP_MATRIX_INIT` | No store, no bootstrap, `/health` false, tools return not-initialized | Yes | T007, T011, T012 | |
| FR-001 | `--features e2ee` on 0.19; non-`e2ee` build still serves unencrypted rooms | Yes | T001, T002, T011, T012, T013, T015 | |
| FR-002 | `CRYPTO_STORE_PATH` persistent volume | Yes | T003, T004 | |
| FR-003 | `.device-id` aligned with `/whoami` | Yes | T002, T004, T005 | |
| FR-004 | Auto-bootstrap unless `SKIP_MATRIX_INIT`; `forceRecreate` local-only | Yes | T007, T011 | |
| FR-005 | Keys only to cross-signed devices | Yes | T008, T014(3) | |
| FR-006 | `encryptionMode` plus pre-flight, fail closed | Yes | T009, T013, T014(4) | |
| FR-007 | Four read/send tools transparent | Yes | T010, T013, T014(1) | |
| FR-008 | No secrets in logs or results; `SecretResolutionError` | Yes | T003, T006, T007, T013 | |
| FR-009 | `/health` always has both booleans | Yes | T011, T012 | Matches constitution v2.0.2 Principle VIII |
| FR-010 | `rmcp` stays 1.2 | Yes | T001 | |
| NFR-001 | Cold bootstrap under 60 s, async `keyBackupRestore` | Yes | T007, T013, T014(6) | |
| NFR-002 | History decrypts after restart | Yes | T004, T005, T014(2) | |
| SC-001 | Two agents, two ids, two stores | Yes | T004, T013 | |
| SC-002 | Continuwuity group room round-trip plus restart | Yes | T009, T010, T014(1-2) | |
| SC-003 | Unverified device cannot decrypt | Yes | T008, T014(3) | |
| SC-004 | Auto-fallback reports `Encrypted: No (fallback: ...)` | Yes | T009, T013, T014(4) | |
| SC-005 | Tests pass on both builds on 0.19 | Yes | T001, T012, T013, T015 | |
| SC-006 | Automated error-path coverage, including `UiaaRequiredError` | Yes | T013 | |

## 3. Constitution Alignment Issues

None. Principles I, II, IV, V, VI, VII, VIII (v2.0.2), IX, and X are satisfied. Locked stack: Rust 1.99 target, compiling pin 1.88 and `matrix-sdk` 0.10 until T001, `matrix-sdk` 0.19.x, `rmcp` 1.2, `matrix-sdk-crypto` >= 0.16.1.

## 4. Unmapped Tasks

None. T001-T015 each map to at least one requirement, story, edge case, success criterion, or the Role in-place constraint.

## 5. Metrics

- Total Requirements (US + Edge Cases + FR + NFR + SC): 27 (5 + 4 + 10 + 2 + 6). The Role sentence is an additional unnumbered constraint and is covered.
- Total Tasks: 15 (T001-T015)
- Coverage % (requirements with >= 1 task): 100% (27 / 27)
- Ambiguity Count: 0
- Duplication Count: 0
- Critical Issues Count: 0
- High Issues Count: 0
- Medium Issues Count: 0
- Low Issues Count: 0

---

## 6. Next Actions

- No CRITICAL, HIGH, MEDIUM, or LOW issues. `/speckit-implement` may proceed from T001.
- Commit the uncommitted Role sentence in `spec.md` (and the `.gitignore` entry for `spec.md.baseline`) before that session, so implementation starts from the text this run checked.
- Do not run `/speckit-plan` or `/speckit-tasks`. Both would regenerate files that already match this spec.
- Remediation edits are not proposed. There are no open findings.
