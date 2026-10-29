# Specification Analysis Report (`/speckit-analyze`)

**Date**: 2026-10-29
**Feature Directory**: `.specify/specs/002-e2ee-cross-signing`
**Artifacts Analyzed**:
- `.specify/memory/constitution.md` (`v2.0.0`)
- `.specify/specs/002-e2ee-cross-signing/spec.md`
- `.specify/specs/002-e2ee-cross-signing/plan.md`
- `.specify/specs/002-e2ee-cross-signing/data-model.md`
- `.specify/specs/002-e2ee-cross-signing/tasks.md`
- `spec/matrix-mcp-v1-v2.yml` (`v2.0.0`)

---

## 1. Findings Table

| ID | Category | Severity | Location(s) | Summary | Recommendation |
|----|----------|----------|-------------|---------|----------------|
| U1 | Underspecification | MEDIUM | `spec.md:L121` (`NFR-001`), `data-model.md:L48`, `tasks.md:L22` (`T007`) | `NFR-001` requires cold bootstrap to complete within 60 seconds by running full historical key-backup download in the background and exposing `keyBackupRestore` state (`not_started \| running \| done \| failed`), but `T007` does not explicitly mention spawning key-backup restore asynchronously or tracking `keyBackupRestore` state. | Clarify in `T007` that historical key-backup restoration runs asynchronously (or with a non-blocking background task updating `keyBackupRestore`) so startup completes within 60s. |
| I1 | Constitution Alignment | LOW | `constitution.md:L44-46` (Principle II), `tasks.md:L29` (`T011`) | Constitution Principle II requires `bootstrap-e2ee` and `get-encryption-status` to remain registered and return a clear `"E2EE support not enabled; rebuild with --features e2ee"` message when compiled without `--features e2ee`. `T011` does not explicitly note the `#[cfg(not(feature = "e2ee"))]` stub handlers. | Note in `T011` and `T012` that `bootstrap-e2ee` and `get-encryption-status` include `#[cfg(not(feature = "e2ee"))]` fallback messages. |
| U2 | Underspecification | LOW | `spec/matrix-mcp-v1-v2.yml:L226-240`, `data-model.md:L24`, `tasks.md:L21-22` (`T006`, `T007`) | `bootstrap-e2ee` accepts `forceRecreate: bool` and `backupPassphraseRef: Option<String>`, but `T006`/`T007` do not state what `backupPassphraseRef` should return when no external secret manager is configured (`SecretResolutionError`), or that `forceRecreate: true` recreates the local SQLite store/device keys while still requiring the recovery secret if an account cross-signing identity exists. | Add a brief note in `T006` (`backupPassphraseRef` returns `SecretResolutionError` until a secret-manager backend is configured) and `T007` (`forceRecreate` rotates local store/device keys without resetting server-side cross-signing). |
| U3 | Underspecification | LOW | `spec.md:L116` (`FR-009`), `tasks.md:L29` (`T011`) | `FR-009` and `T011` state `/health` includes `e2ee` and `crossSigned` booleans when the `e2ee` feature is on, but do not specify whether `/health` omits those keys or returns `"e2ee": false, "crossSigned": false` when compiled without `--features e2ee` or when `E2EE_ENABLED=false`. | Specify in `T011` that `GET /health` always includes `"e2ee": <bool>` and `"crossSigned": <bool>` (both `false` when feature is disabled or `E2EE_ENABLED=false`) for deterministic `am-ctl` health checks. |

---

## 2. Coverage Summary Table

| Requirement / Story Key | Summary | Has Task? | Task IDs | Notes |
|-------------------------|---------|-----------|----------|-------|
| **US1** (`spec.md:L36-47`) | Per-agent device id (`.device-id` via `/whoami`) and isolated SQLite store | Yes | `T002`, `T003`, `T004`, `T005`, `T013` | Complete |
| **US2** (`spec.md:L49-60`) | Startup auto-bootstrap & `bootstrap-e2ee` tool using recovery secret | Yes | `T006`, `T007`, `T011`, `T013` | Complete (see `U1`/`U2` for minor detail refinements) |
| **US3** (`spec.md:L62-78`) | E2EE-by-default room creation (`encryptionMode="auto"`) with `/keys/query` fallback | Yes | `T009`, `T013`, `T014` | Complete |
| **US4** (`spec.md:L80-94`) | Transparent messaging & reading across encrypted and unencrypted rooms | Yes | `T008`, `T010`, `T013`, `T014` | Complete |
| **US5** (`spec.md:L96-98`) | `get-encryption-status` tool and `/health` `e2ee` & `crossSigned` fields | Yes | `T011`, `T013` | Complete (see `I1`/`U3`) |
| **FR-001** (`spec.md:L108`) | `--features e2ee` against `matrix-sdk` 0.19.x; non-`e2ee` build compiles | Yes | `T001`, `T002`, `T012`, `T013` | Complete |
| **FR-002** (`spec.md:L109`) | Persistent `CRYPTO_STORE_PATH` volume mount | Yes | `T003`, `T004` | Complete |
| **FR-003** (`spec.md:L110`) | `.device-id` in `$CRYPTO_STORE_PATH/.device-id` aligned with `/whoami` | Yes | `T002`, `T004`, `T005` | Complete |
| **FR-004** (`spec.md:L111`) | Auto-bootstrap on startup when `E2EE_ENABLED=true` + `bootstrap-e2ee` tool | Yes | `T007`, `T011` | Complete |
| **FR-005** (`spec.md:L112`) | Outbound Megolm keys go only to cross-signed devices (no TOFU) | Yes | `T008`, `T014` | Complete |
| **FR-006** (`spec.md:L113`) | `create-room` & `send-direct-message` `encryptionMode` (`auto`/`e2ee`/`none`) + `/keys/query` fallback | Yes | `T009`, `T013`, `T014` | Complete |
| **FR-007** (`spec.md:L114`) | `send-message`, `get-room-messages`, `get-messages-by-date`, `identify-active-users` transparent E2EE | Yes | `T010`, `T013`, `T014` | Complete |
| **FR-008** (`spec.md:L115`) | Recovery material and passwords never appear in logs or tool results | Yes | `T003`, `T006`, `T007` | Complete |
| **FR-009** (`spec.md:L116`) | `/health` includes `e2ee` and `crossSigned` booleans | Yes | `T011` | Complete |
| **FR-010** (`spec.md:L117`) | `rmcp` remains 1.2 | Yes | `T001` | Complete |
| **NFR-001** (`spec.md:L121`) | Cold bootstrap < 60s; historical key backup runs in background | Partial | `T007`, `T011` | Flagged in `U1` |
| **NFR-002** (`spec.md:L122`) | Decryptability preserved across container restart with same volume | Yes | `T004`, `T005`, `T014` | Complete |
| **SC-001** (`spec.md:L126`) | Two agents, two device ids, two isolated stores | Yes | `T004`, `T013` | Complete |
| **SC-002** (`spec.md:L127`) | Continuwuity encrypted group room round-trip with Element + restart | Yes | `T010`, `T014` | Complete |
| **SC-003** (`spec.md:L128`) | Non-cross-signed device cannot decrypt (`m.room_key.withheld`) | Yes | `T008`, `T014` | Complete |
| **SC-004** (`spec.md:L129`) | `create-room` (`auto`) fallback with unbootstrapped invitee + unencrypted rooms work | Yes | `T009`, `T013`, `T014` | Complete |
| **SC-005** (`spec.md:L130`) | Tests pass on both non-`e2ee` and `--features e2ee` builds on `0.19.x` | Yes | `T012`, `T013` | Complete |

---

## 3. Constitution Alignment Issues

- **Zero CRITICAL or HIGH constitution conflicts.**
- All 10 principles in `.specify/memory/constitution.md` (`v2.0.0`) align with `spec.md`, `plan.md`, `data-model.md`, `tasks.md`, and `spec/matrix-mcp-v1-v2.yml`.
- One `LOW` detail note (`I1`): explicitly mention the `#[cfg(not(feature = "e2ee"))]` stub responses for `bootstrap-e2ee` and `get-encryption-status` in `T011` to match Principle II (`constitution.md:L44-46`).

## 4. Unmapped Tasks

- **None** (`0` unmapped tasks; all `T001`–`T014` trace directly to one or more `FR-###`, `NFR-###`, `SC-###`, or `US#` items).

## 5. Metrics

- **Total Requirements (`FR` + `NFR` + `SC` + `US`)**: 22
- **Total Tasks**: 14 (`T001`–`T014`)
- **Coverage % (requirements with >= 1 task)**: **100%** (`22 / 22`)
- **Critical Issues Count**: **0**
- **High Issues Count**: **0**
- **Medium Issues Count**: **1** (`U1`)
- **Low Issues Count**: **3** (`I1`, `U2`, `U3`)
- **Duplication Count**: **0**

---

## 6. Next Actions & Remediation Status

- **Remediation Applied (2026-10-29)**: All 4 findings (`U1`, `I1`, `U2`, `U3`) have been incorporated into `T006`, `T007`, `T011`, and `T012` in `.specify/specs/002-e2ee-cross-signing/tasks.md`.
- **Current Issue Count**: `0 CRITICAL`, `0 HIGH`, `0 MEDIUM`, `0 LOW` (`100%` requirement-to-task coverage).
- **Ready for Implementation**: The specification set is clean and ready for `/speckit-implement` (starting with `T001` + `T002`: `matrix-sdk` 0.19 migration).
