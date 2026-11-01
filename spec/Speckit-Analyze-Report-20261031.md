# Specification Analysis Report (`/speckit-analyze` -- Run 3)

**Date**: 2026-10-31
**Feature Directory**: `.specify/specs/002-e2ee-cross-signing`
**Inputs**: `.specify/memory/constitution.md` (v2.0.1, amended 2026-10-31), `spec.md`, `plan.md`, `data-model.md`, `tasks.md`
**Prerequisite check**: `check-prerequisites.sh --json --require-spec --require-tasks --include-tasks` -> `AVAILABLE_DOCS: ["data-model.md","tasks.md"]`. No `.specify/extensions.yml` hooks.
**Trigger**: Constitution amendment (Principle V hostile-network paragraph; locked Rust MSRV 1.96 -> 1.99) and the 2026-10-30 reconciliation edits (`SC-006`, `SKIP_MATRIX_INIT`, expanded T013/T014). Supersedes `spec/Speckit-Analyze-Report-20261029.md` (Run 2).

---

## 1. Findings

| ID | Category | Severity | Location(s) | Summary | Recommendation |
|----|----------|----------|-------------|---------|----------------|
| C1 | Constitution Alignment | MEDIUM | `constitution.md` Principle VII ("Both the feature-enabled build and the feature-disabled build MUST pass CI"); `.github/workflows/{docker-publish,release-binary}.yml`; `tasks.md` T012-T013 | No CI workflow runs `cargo test`, `cargo clippy`, or `cargo fmt` for either feature build. T012/T013 verify locally but no task adds the CI matrix the constitution requires. | Add to T012 or a new T015: a GitHub Actions job running `cargo fmt --check`, `cargo clippy`, `cargo test` and `cargo test --features e2ee` on `rust:1.99`. |
| U1 | Underspecification | LOW | `tasks.md:15` (T003), `plan.md:49`, `spec.md:58` (US2 AC2) | The "optional UIAA password env/file reference" is never named. Implementers will invent a variable name; operators cannot document it. | Name it in `data-model.md` Identity & Secrets and T003 (for example `MATRIX_UIAA_PASSWORD_FILE`, file path only, 0600, never logged). |
| I1 | Inconsistency | LOW | `constitution.md` Principle VIII ("`/health` MUST reflect `e2ee` and `crossSigned` status when E2EE is enabled") vs `spec.md` FR-009 ("always includes ... both `false` when disabled") | Spec is a strict superset of the constitution wording, so not a conflict, but a reader of the constitution alone would accept a `/health` that omits the keys when E2EE is off. | Optional PATCH wording in the constitution at the next amendment: "MUST always include `e2ee` and `crossSigned` booleans". No spec change needed. |

No CRITICAL or HIGH findings. Duplication: none. Unresolved placeholders: none. Vague adjectives without measurable criteria: none (the "clear notice" / "clear marker" phrases are pinned by the exact strings `Encrypted: No (fallback: ...)` and `[Encrypted message - unable to decrypt (missing_key)]`).

### Verification of the MSRV amendment

Constitution v2.0.1 raised the E2EE target MSRV to 1.99. Propagated and verified consistent in: `spec.md:8, 27`; `plan.md:11, 52-53`; `tasks.md:10` (T001); `Cargo.toml:10, 26` (comments); `src/AGENTS.md:7`; `.cursor/rules/rust-e2ee.mdc:15`; `.cursor/rules/project-context.mdc:22`; `spec/E2EE-Key-Management-Model.md:26`; `spec/E2EE-Landscape-Review-October-20261023.md:17, 33, 140`. Remaining `1.96` mentions are factual statements of the `matrix-sdk` 0.19 crate floor, labeled as such. Docker Hub tag `rust:1.99-bookworm` exists (checked 2026-10-31). Historical reports (`E2EE-Repository-Audit-20261029.md`, `Agent-Matrix-E2EE-Context-Synthesis-20261029.md`, `E2EE-Landscape-Review-20261023.md`) retain 1.96 as dated records.

---

## 2. Coverage Summary

| Requirement Key | Summary | Has Task? | Task IDs | Notes |
|-----------------|---------|-----------|----------|-------|
| US1 (`spec.md:36-47`) | Per-agent `.device-id` via `/whoami`, isolated store | Yes | T002, T003, T004, T005, T013 | |
| US2 (`spec.md:49-60`) | Startup auto-bootstrap and `bootstrap-e2ee`, recovery secret, `IdentityExistsError`, `trustPolicy` | Yes | T006, T007, T011, T013, T014(6) | UIAA variable unnamed (U1) |
| US3 (`spec.md:62-78`) | `encryptionMode` auto / e2ee / none with `/keys/query` pre-flight | Yes | T009, T013, T014(4) | |
| US4 (`spec.md:80-94`) | Transparent send/read, cross-signed-only key share, UTD marker | Yes | T008, T010, T013, T014(1-3) | |
| US5 (`spec.md:96-98`) | `get-encryption-status`, `/health`, non-`e2ee` stubs, `SKIP_MATRIX_INIT` | Yes | T011, T012, T013 | |
| Edge: wiped store (`spec.md:102`) | Align `.device-id` before `/keys/upload` | Yes | T004, T007 | |
| Edge: two processes (`spec.md:103`) | `KeyStoreUnavailableError` lock | Yes | T004, T013 | |
| Edge: encrypted DMs (`spec.md:104`) | DM gate recorded as homeserver result | Yes | T014(5) | |
| Edge: `SKIP_MATRIX_INIT` (`spec.md:105`) | No store, no bootstrap, `/health` false, tools return not-initialized | Yes | T007, T011, T012 | |
| FR-001 | `--features e2ee` on 0.19; non-`e2ee` stubs | Yes | T001, T002, T011, T012, T013 | |
| FR-002 | `CRYPTO_STORE_PATH` persistent volume | Yes | T003, T004 | |
| FR-003 | `.device-id` aligned with `/whoami` | Yes | T002, T004, T005 | |
| FR-004 | Auto-bootstrap unless `SKIP_MATRIX_INIT`; `forceRecreate` local-only | Yes | T007, T011 | |
| FR-005 | Keys only to cross-signed devices | Yes | T008, T014(3) | |
| FR-006 | `encryptionMode` + pre-flight, fail closed | Yes | T009, T013, T014(4) | |
| FR-007 | Four read/send tools transparent | Yes | T010, T013, T014(1) | |
| FR-008 | No secrets in logs/results; `SecretResolutionError` | Yes | T003, T006, T007, T013 | |
| FR-009 | `/health` always has both booleans | Yes | T011, T012 | |
| FR-010 | `rmcp` stays 1.2 | Yes | T001 | |
| NFR-001 | Cold bootstrap < 60 s, async `keyBackupRestore` | Yes | T007, T013, T014(6) | |
| NFR-002 | History decrypts after restart | Yes | T004, T005, T014(2) | |
| SC-001 | Two agents, two ids, two stores | Yes | T004, T013 | |
| SC-002 | Continuwuity group room round-trip + restart | Yes | T009, T010, T014(1-2) | |
| SC-003 | Unverified device cannot decrypt | Yes | T008, T014(3) | |
| SC-004 | Auto-fallback reports `Encrypted: No (fallback: ...)`; unencrypted rooms unchanged | Yes | T009, T013, T014(4) | |
| SC-005 | Tests pass on both builds on 0.19 | Yes | T001, T012, T013 | CI matrix missing (C1) |
| SC-006 | Automated error-path coverage | Yes | T013 | |

## 3. Constitution Alignment Issues

- MEDIUM: C1 (Principle VII CI requirement has no task).
- LOW: I1 (Principle VIII wording narrower than FR-009; spec is stricter, no conflict).
- Principles I, II, IV, V (including the new hostile-network paragraph), VI, IX, X: satisfied. Locked stack (Rust 1.99, `matrix-sdk` 0.19.x, `rmcp` 1.2, `matrix-sdk-crypto` >= 0.16.1): consistent across spec, plan, tasks, `Cargo.toml` comments, and rules.

## 4. Unmapped Tasks

None. T001-T014 each map to at least one requirement, story, edge case, or success criterion.

## 5. Metrics

- Total Requirements (US + Edge Cases + FR + NFR + SC): 27 (5 + 4 + 10 + 2 + 6)
- Total Tasks: 14 (T001-T014)
- Coverage % (requirements with >= 1 task): 100% (27 / 27)
- Ambiguity Count: 0
- Duplication Count: 0
- Critical Issues Count: 0
- High Issues Count: 0
- Medium Issues Count: 1 (C1)
- Low Issues Count: 2 (U1, I1)

---

## 6. Next Actions

- No CRITICAL or HIGH issues; `/speckit-implement` may proceed from T001.
- Recommended before implementing (small edits, not blocking): add a CI matrix task for both feature builds (C1); name the UIAA password variable in `data-model.md` and T003 (U1).
- Optional at the next constitution amendment: tighten Principle VIII to "always include" (I1).
- Remediation applied 2026-10-31 after owner approval: C1 -> new T015 (`.github/workflows/ci.yml`, fmt/clippy/test matrix on both feature builds; T012/T013 block T015) plus plan.md Constitution Check and source-map entries; U1 -> `MATRIX_UIAA_PASSWORD_FILE` named in `data-model.md` (UiaaPassword), T003, T014(6), and `plan.md`. I1 left for a future constitution amendment. Task count is now 15.
