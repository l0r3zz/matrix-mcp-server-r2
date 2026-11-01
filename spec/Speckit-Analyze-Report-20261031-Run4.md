# Specification Analysis Report (`/speckit-analyze` -- Run 4)

**Date**: 2026-10-31
**Feature Directory**: `.specify/specs/002-e2ee-cross-signing`
**Inputs**: `.specify/memory/constitution.md` (v2.0.1), `spec.md`, `plan.md`, `data-model.md`, `tasks.md` (15 tasks), `spec/matrix-mcp-v1-v2.yml`, `Cargo.toml` (features)
**Prerequisite check**: `check-prerequisites.sh --json --require-spec --require-tasks --include-tasks` -> `AVAILABLE_DOCS: ["data-model.md","tasks.md"]`. No `.specify/extensions.yml` hooks.
**Trigger**: Verification run after applying Run 3 remediation (commit `3e7ec0b`): T015 CI matrix (C1) and `MATRIX_UIAA_PASSWORD_FILE` (U1). Supersedes `spec/Speckit-Analyze-Report-20261031.md` (Run 3).

---

## 1. Findings

| ID | Category | Severity | Location(s) | Summary | Recommendation |
|----|----------|----------|-------------|---------|----------------|
| I1 | Inconsistency | LOW | `constitution.md` Principle VIII vs `spec.md` FR-009 | Carried from Run 3. Constitution says `/health` reflects `e2ee`/`crossSigned` "when E2EE is enabled"; spec says always. Spec is stricter; no conflict. | Optional PATCH wording at the next constitution amendment. |
| I2 | Inconsistency | LOW | `tasks.md` T015 text vs `tasks.md` Dependencies line | T015 says "Both feature builds must pass before T014 is recorded as complete", but the Dependencies line only states "T012 and T013 block T015" and does not list T015 as a prerequisite for T014 sign-off. | Append "T015 blocks T014 sign-off" to the Dependencies line so the ordering is stated once, explicitly. |
| U2 | Underspecification | LOW | `tasks.md` T015 | The job runs `cargo fmt`, `cargo clippy`, and two `cargo test` invocations, but does not say whether `clippy` runs on both feature builds. `fmt` is feature-independent; `clippy` is not (`#[cfg(feature = "e2ee")]` code is only linted when the feature is on). | State: `fmt` once; `clippy --all-targets -- -D warnings` and `cargo test` per matrix entry (`""` and `--features e2ee`). |
| I3 | Inconsistency | LOW | `spec/matrix-mcp-v1-v2.yml:29` vs `data-model.md` UiaaPassword | The contract's principles list names the three recovery-secret sources (including `MATRIX_RECOVERY_KEY_FILE`) but not `MATRIX_UIAA_PASSWORD_FILE`. Principle X requires the contract and spec to stay consistent. Not a tool-input change, so no schema impact. | Optional one-line addition next to line 29: "Initial Secret Storage creation may read an account password from `MATRIX_UIAA_PASSWORD_FILE` for UIAA only; it is never a tool input." |

No CRITICAL, HIGH, or MEDIUM findings. Duplication: none. Unresolved placeholders: none. Vague adjectives without measurable criteria: none.

### Verification of Run 3 remediation

- **C1 (closed)**: T015 adds `.github/workflows/ci.yml` on `rust:1.99` running fmt, clippy, `cargo test`, and `cargo test --features e2ee`. `plan.md` Constitution Check now cites Principle VII with T015, and the source map lists `ci.yml`. Principle VII ("Both the feature-enabled build and the feature-disabled build MUST pass CI") now has task coverage. Residual: U2 above.
- **U1 (closed)**: `MATRIX_UIAA_PASSWORD_FILE` is defined once in `data-model.md` (UiaaPassword: 0600 file, UIAA on `/keys/device_signing/upload` only, never logged/returned/accepted as an MCP argument) and referenced consistently in T003, T014(6), and `plan.md:50`. `spec.md:58` (US2 AC2) still says "optional UIAA password" without the variable name; this is acceptable because the spec names behavior and the data model names configuration, matching how `MATRIX_RECOVERY_KEY_FILE` is handled.
- **SC-005 wording**: `Cargo.toml` has `default = []`, so `--no-default-features` (SC-005) and plain `cargo test` (T012, T015) are the same build today. Not a finding; noted so a future default-feature change does not silently desynchronize them.

---

## 2. Coverage Summary

| Requirement Key | Summary | Has Task? | Task IDs | Notes |
|-----------------|---------|-----------|----------|-------|
| US1 (`spec.md:36-47`) | Per-agent `.device-id` via `/whoami`, isolated store | Yes | T002, T003, T004, T005, T013 | |
| US2 (`spec.md:49-60`) | Startup auto-bootstrap and `bootstrap-e2ee`, recovery secret, UIAA, `IdentityExistsError`, `trustPolicy` | Yes | T003, T006, T007, T011, T013, T014(6) | UIAA variable now named (U1 closed) |
| US3 (`spec.md:62-78`) | `encryptionMode` auto / e2ee / none with `/keys/query` pre-flight | Yes | T009, T013, T014(4) | |
| US4 (`spec.md:80-94`) | Transparent send/read, cross-signed-only key share, UTD marker | Yes | T008, T010, T013, T014(1-3) | |
| US5 (`spec.md:96-98`) | `get-encryption-status`, `/health`, non-`e2ee` stubs, `SKIP_MATRIX_INIT` | Yes | T011, T012, T013 | |
| Edge: wiped store (`spec.md:102`) | Align `.device-id` before `/keys/upload` | Yes | T004, T007 | |
| Edge: two processes (`spec.md:103`) | `KeyStoreUnavailableError` lock | Yes | T004, T013 | |
| Edge: encrypted DMs (`spec.md:104`) | DM gate recorded as homeserver result | Yes | T014(5) | |
| Edge: `SKIP_MATRIX_INIT` (`spec.md:105`) | No store, no bootstrap, `/health` false, tools return not-initialized | Yes | T007, T011, T012 | |
| FR-001 | `--features e2ee` on 0.19; non-`e2ee` stubs | Yes | T001, T002, T011, T012, T013, T015 | |
| FR-002 | `CRYPTO_STORE_PATH` persistent volume | Yes | T003, T004 | |
| FR-003 | `.device-id` aligned with `/whoami` | Yes | T002, T004, T005 | |
| FR-004 | Auto-bootstrap unless `SKIP_MATRIX_INIT`; `forceRecreate` local-only | Yes | T007, T011 | |
| FR-005 | Keys only to cross-signed devices | Yes | T008, T014(3) | |
| FR-006 | `encryptionMode` + pre-flight, fail closed | Yes | T009, T013, T014(4) | |
| FR-007 | Four read/send tools transparent | Yes | T010, T013, T014(1) | |
| FR-008 | No secrets in logs/results; `SecretResolutionError` | Yes | T003, T006, T007, T013 | Now includes `MATRIX_UIAA_PASSWORD_FILE` |
| FR-009 | `/health` always has both booleans | Yes | T011, T012 | |
| FR-010 | `rmcp` stays 1.2 | Yes | T001 | |
| NFR-001 | Cold bootstrap < 60 s, async `keyBackupRestore` | Yes | T007, T013, T014(6) | |
| NFR-002 | History decrypts after restart | Yes | T004, T005, T014(2) | |
| SC-001 | Two agents, two ids, two stores | Yes | T004, T013 | |
| SC-002 | Continuwuity group room round-trip + restart | Yes | T009, T010, T014(1-2) | |
| SC-003 | Unverified device cannot decrypt | Yes | T008, T014(3) | |
| SC-004 | Auto-fallback reports `Encrypted: No (fallback: ...)`; unencrypted rooms unchanged | Yes | T009, T013, T014(4) | |
| SC-005 | Tests pass on both builds on 0.19 | Yes | T001, T012, T013, T015 | CI matrix added (C1 closed) |
| SC-006 | Automated error-path coverage | Yes | T013 | |

## 3. Constitution Alignment Issues

- LOW: I1 (Principle VIII wording narrower than FR-009; spec is stricter, no conflict).
- LOW: I3 (Principle X contract/spec consistency; `MATRIX_UIAA_PASSWORD_FILE` absent from the contract's notes).
- Principles I, II, IV, V, VI, VII (now covered by T015), IX, X: satisfied. Locked stack (Rust 1.99, `matrix-sdk` 0.19.x, `rmcp` 1.2, `matrix-sdk-crypto` >= 0.16.1): consistent across spec, plan, tasks, `Cargo.toml` comments, and rules.

## 4. Unmapped Tasks

None. T001-T015 each map to at least one requirement, story, edge case, or success criterion. T015 maps to FR-001, SC-005, and Principle VII.

## 5. Metrics

- Total Requirements (US + Edge Cases + FR + NFR + SC): 27 (5 + 4 + 10 + 2 + 6)
- Total Tasks: 15 (T001-T015)
- Coverage % (requirements with >= 1 task): 100% (27 / 27)
- Ambiguity Count: 0
- Duplication Count: 0
- Critical Issues Count: 0
- High Issues Count: 0
- Medium Issues Count: 0
- Low Issues Count: 4 (I1, I2, U2, I3)

---

## 6. Next Actions

- No CRITICAL, HIGH, or MEDIUM issues; `/speckit-implement` may proceed from T001.
- Optional pre-implementation edits (two lines in `tasks.md`, one in `spec/matrix-mcp-v1-v2.yml`): I2, U2, I3.
- Optional at the next constitution amendment: I1.
- Remediation edits are not applied by this report; they await owner approval.
