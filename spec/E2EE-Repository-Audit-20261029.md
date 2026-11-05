# Read-Only Repository Audit: `matrix-mcp-server-r2`

**Date**: 2026-10-29
**Branch Audited**: `002-e2ee-cross-signing` (commit `fa70f4a`)

## 1. What This Project Appears to Be Building

- **Observed implementation:** `matrix-mcp-server-r2` (currently at v0.1.1 in `Cargo.toml:3`) is a Rust HTTP server using `axum` 0.8 (`src/main.rs:84-96`) and `rmcp` 1.2's `StreamableHttpService` at `/mcp` (`src/main.rs:71-80`) plus a `GET /health` endpoint (`src/main.rs:85-95`). It connects a single shared `matrix-sdk` 0.10 client (`Cargo.toml:31`, `src/matrix/client.rs:18-57`) with a background sync loop (`src/matrix/client.rs:60-74`) and exposes 20 unencrypted Matrix v1 MCP tools (`src/mcp/server.rs:256-660`) as a drop-in replacement for the reference TypeScript `matrix-mcp-server` (`README.md:3-4`).
- **Documented requirement:** Phase 3 ("v2 Implementation -- E2EE") adds end-to-end encrypted Matrix room and message support for the Agent-Matrix fleet (`README.md:13`, `.specify/specs/002-e2ee-cross-signing/spec.md:1-12`). It requires:
  - Keeping all 20 v1 tools backwards-compatible on unencrypted rooms while refusing encrypted rooms with `EncryptedRoomError` (`"room is encrypted; use *-r2 tools instead"`) (`.specify/memory/constitution.md:20-27`, `spec/matrix-mcp-v1-v2.yml:67-72`).
  - Adding five v2 (`-r2`) tools gated behind `--features e2ee`: `bootstrap-e2ee-r2`, `create-room-r2`, `send-message-r2`, `get-room-messages-r2`, and `get-encryption-status-r2` (`spec/matrix-mcp-v1-v2.yml:203-367`, `.specify/specs/002-e2ee-cross-signing/spec.md:90-99`).
  - Migrating from `matrix-sdk` 0.10 (Rust 1.88) to `matrix-sdk` 0.19.x (Rust >= 1.96) with `e2e-encryption` and `bundled-sqlite`, per-agent persistent SQLite crypto stores (`CRYPTO_STORE_PATH`) and `.device-id` files, Secret Storage recovery key resolution, and strict **cross-signed-only** device trust (`TOFU` forbidden) against Continuwuity homeservers (`.specify/specs/002-e2ee-cross-signing/spec.md:22-33, 89-100`).

---

## 2. Populated Project Documents vs. Unfilled Templates

### Populated Project Documents

| Path | Status / Nature |
|---|---|
| `README.md` | Populated project overview, architecture, v1 tool table, GHCR publishing, and test instructions (`README.md:1-193`). |
| `.specify/memory/constitution.md` | Populated project constitution (v1.1.0, 10 principles, locked tech stack, workflow, governance; note: contains an uncommitted Sync Impact Report comment at lines 1-13). |
| `.specify/specs/002-e2ee-cross-signing/spec.md` | Populated active feature specification (Created 2026-10-23, Status: Accepted) (`.specify/specs/002-e2ee-cross-signing/spec.md:1-118`). |
| `.specify/specs/002-e2ee-cross-signing/plan.md` | Populated implementation plan for `002-e2ee-cross-signing` (`.specify/specs/002-e2ee-cross-signing/plan.md:1-60`). |
| `.specify/specs/002-e2ee-cross-signing/data-model.md` | Populated data model for `002-e2ee-cross-signing` (`.specify/specs/002-e2ee-cross-signing/data-model.md:1-52`). |
| `.specify/specs/002-e2ee-cross-signing/tasks.md` | Populated 15-task breakdown (`T001`–`T015`, all unchecked `[ ]`) (`.specify/specs/002-e2ee-cross-signing/tasks.md:1-40`). |
| `.specify/specs/001-e2ee-support/spec.md` | Populated May 2026 E2EE specification, explicitly marked **Superseded** at lines 5-7. |
| `spec/matrix-mcp-v1-v2.yml` | Populated YAML API contract defining concepts, 4 of 20 v1 tools, 5 v2 `-r2` tools, and test suites (`spec/matrix-mcp-v1-v2.yml:1-391`). |
| `spec/E2EE-Key-Management-Model.md` | Populated key management & bootstrap design doc (`spec/E2EE-Key-Management-Model.md:1-238`). |
| `spec/E2EE-Landscape-Review-October-20261023.md` | Populated October 2026 E2EE architecture/landscape briefing (`spec/E2EE-Landscape-Review-October-20261023.md:1-172`). |
| `spec/E2EE-Landscape-Review-20261023.md` | Populated earlier snapshot of the landscape review (`spec/E2EE-Landscape-Review-20261023.md:1-150`). |
| `spec.bak/*` (5 files) | Untracked byte-for-byte duplicate backup of `spec/*`. |
| `src/AGENTS.md` | Populated project guide for `src/` (`src/AGENTS.md:1-112`). |
| `.cursor/rules/*.mdc` (5 files) | Populated Cursor rules (`project-context.mdc`, `rust-e2ee.mdc`, `matrix-client.mdc`, `mcp-tools.mdc`, `rust-conventions.mdc`). |
| `.a0proj/instructions/project-instructions.md` | Populated Agent Zero instructions (from earlier Phase 2 milestone) (`.a0proj/instructions/project-instructions.md:1-63`). |
| `reference/docs/*` & `reference/ts-source/*` | Populated frozen reference docs and TypeScript implementation snapshot. |

### Unfilled Templates & Partially Completed Checklists

| Path | Status / Nature |
|---|---|
| `.specify/templates/spec-template.md` | **Unfilled template.** Uncommitted working-tree version is generic Spec Kit v1.2.3 (`[FEATURE NAME]`, `[###-feature-name]`); committed `HEAD` version was a shortened template. |
| `.specify/templates/plan-template.md` | **Unfilled template.** Uncommitted working-tree version is generic Spec Kit v1.2.3; committed `HEAD` version had Rust/matrix-sdk 0.10 placeholder examples (`[e.g., Rust 1.88, 2021 edition]`). |
| `.specify/templates/tasks-template.md` | **Unfilled template.** Generic Spec Kit v1.2.3 sample tasks (`T001`..`T028`, `src/models/[entity1].py`). |
| `.specify/templates/constitution-template.md` | **Unfilled template** (untracked, added by Spec Kit v1.2.3 init: `[PROJECT_NAME]`, `[PRINCIPLE_1_NAME]`). |
| `.specify/templates/checklist-template.md` | **Unfilled template** (untracked, added by Spec Kit v1.2.3 init: `[CHECKLIST TYPE]`, `CHK001`). |
| `spec/E2EE-Prep-Checklist-for-Rust-MCP-Server.md` | **Mostly unfilled checklist.** Out of ~75 checkboxes, only 4 are checked (`[x]` at lines 230-231 and 271-272), and lines 230-231 check `[x] TOFU`, which contradicts the October design. |
| `AGENTS.md` (repo root) | **Generic DOX framework instructions with an unfilled placeholder** at lines 81-83: `"This project is not yet indexed. Before continuing you must scan the project, build the DOX tree and replace this message with the actual index."` |

---

## 3. Installed Spec Kit Commands and Exact Invocation Syntax

- **Observed installation:** Spec Kit v1.2.3 is initialized with the `cursor-agent` integration (`.specify/init-options.json:1-10`, `.specify/integration.json:1-16`, `.specify/integrations/cursor-agent.manifest.json:1-18`).
- **Separator configuration:** `.specify/integration.json:10` sets `"invoke_separator": "-"`, and `.specify/scripts/bash/common.sh:369-375` formats commands as `/speckit-<command>`.
- **Installed skills (10 total):** Located in `.cursor/skills/speckit-*/SKILL.md` (untracked in Git):
  1. `/speckit-constitution [principles / governance input]` -- `.cursor/skills/speckit-constitution/SKILL.md:1-180`
  2. `/speckit-specify <feature description>` -- `.cursor/skills/speckit-specify/SKILL.md:1-346`
  3. `/speckit-clarify [optional focus]` -- `.cursor/skills/speckit-clarify/SKILL.md:1-292`
  4. `/speckit-plan [technical context / arguments]` -- `.cursor/skills/speckit-plan/SKILL.md:1-167`
  5. `/speckit-tasks [optional arguments]` -- `.cursor/skills/speckit-tasks/SKILL.md:1-216`
  6. `/speckit-analyze [optional arguments]` -- `.cursor/skills/speckit-analyze/SKILL.md:1-260`
  7. `/speckit-checklist [checklist type / domain]` -- `.cursor/skills/speckit-checklist/SKILL.md:1-384`
  8. `/speckit-implement [optional arguments]` -- `.cursor/skills/speckit-implement/SKILL.md:1-227`
  9. `/speckit-converge [optional arguments]` -- `.cursor/skills/speckit-converge/SKILL.md:1-284`
  10. `/speckit-taskstoissues` *(marked Deprecated at `.cursor/skills/speckit-taskstoissues/SKILL.md:11-17`)*

*(Note: No `.cursor/commands/` directory exists; commands are installed strictly as Cursor Agent Skills under `.cursor/skills/`.)*

---

## 4. Which Feature Directory the Workflow Would Currently Operate On (Critical Finding)

- **Observed state:** **None -- the Spec Kit v1.2.3 workflow scripts will currently fail with an error, or look in the wrong root directory (`/specs/` instead of `/.specify/specs/`).**
- **Evidence from `.specify/scripts/bash/`:**
  1. **Missing `.specify/feature.json` and no Git-branch fallback:** In Spec Kit v1.2.3, `get_feature_paths()` in `.specify/scripts/bash/common.sh:187-215` resolves the active feature directory strictly from:
     - Priority 1: `SPECIFY_FEATURE_DIRECTORY` environment variable (`common.sh:192`)
     - Priority 2: `.specify/feature.json` `"feature_directory"` key (`common.sh:201`)
     - Priority 3: Hard error (`"ERROR: Feature directory not found. Set SPECIFY_FEATURE_DIRECTORY or run the specify command to create .specify/feature.json."` at `common.sh:213-214`).
     It **does not** inspect `git branch --show-current` (`002-e2ee-cross-signing`). Because neither `SPECIFY_FEATURE_DIRECTORY` nor `.specify/feature.json` exists in this checkout, running `setup-plan.sh`, `setup-tasks.sh`, or `check-prerequisites.sh` (used by `/speckit-plan`, `/speckit-tasks`, `/speckit-analyze`, `/speckit-implement`, `/speckit-converge`, `/speckit-clarify`, `/speckit-checklist`) will immediately exit 1.
  2. **Directory path mismatch (`specs/` vs `.specify/specs/`):**
     - Existing feature directories in this repository live at **`.specify/specs/001-e2ee-support/`** and **`.specify/specs/002-e2ee-cross-signing/`**.
     - However, Spec Kit v1.2.3's `create-new-feature.sh:279` hardcodes `SPECS_DIR="$REPO_ROOT/specs"`, and `.cursor/skills/speckit-specify/SKILL.md:81-91` auto-generates new features under top-level `specs/<NNN>-<short-name>` by scanning `specs/`. Because top-level `specs/` does not exist in this repo, `/speckit-specify` would create `specs/001-...` (colliding in number with `.specify/specs/001-e2ee-support`).
- **Inferred intent:** The human and previous agent intended the active feature to be `.specify/specs/002-e2ee-cross-signing` (matching checked-out branch `002-e2ee-cross-signing` and commit `fa70f4a`).

---

## 5. Apparent Implementation Status

### By Code Inspection

1. **v1 Unencrypted Server (Phase 2): Implemented.**
   - `src/main.rs:17-134` wires CLI `--version`, `tracing_subscriber`, `Config::from_env()`, `SKIP_MATRIX_INIT` stub mode, `matrix::create_matrix_client`, `start_background_sync`, `ClientCache`, `StreamableHttpService` at `/mcp`, `GET /health` at `/health`, `CorsLayer`, and graceful shutdown on `ctrl_c`.
   - `src/mcp/server.rs:256-660` implements all 20 v1 `#[tool]` methods (`list_joined_rooms`, `get_room_info`, `get_room_members`, `get_room_messages`, `get_messages_by_date`, `identify_active_users`, `get_user_profile`, `get_my_profile`, `get_all_users`, `search_public_rooms`, `get_notification_counts`, `get_direct_messages`, `send_message`, `send_direct_message`, `create_room`, `join_room`, `leave_room`, `invite_user`, `set_room_name`, `set_room_topic`).
2. **Dead / Unwired Code in v1:**
   - `src/auth.rs:5-20` defines `MatrixAuthContext` and `from_config()`, which are never referenced anywhere in `src/` or `tests/`.
   - `src/matrix/cache.rs:20-97` defines `ClientCache`. In `src/main.rs:55-63`, a single client is inserted via `cache.set(...)` and the cleanup task is spawned, but `cache.get()`, `cache.remove()`, and `cache.shutdown_all()` are never called in production code (all tools use `self.client` directly).
   - `src/config.rs:14-27` parses OAuth, token-exchange, and HTTPS/SSL environment variables (`enable_oauth`, `enable_token_exchange`, `enable_https`, `ssl_key_path`, `ssl_cert_path`, `idp_*`), but none of them are used anywhere in `src/`.
   - `src/mcp/server.rs:116-117, 505-521`: `SendMessageInput` accepts `reply_to_event_id`, logs `"(reply to ...)"` in the text response (`server.rs:520`), but never attaches a Matrix reply relation to `RoomMessageEventContent`.
   - `src/mcp/server.rs:160, 614-622`: `LeaveRoomInput` accepts `reason`, logs it in the text response (`server.rs:621`), but `room.leave().await` (`server.rs:617`) does not pass the reason to Matrix.
3. **v2 E2EE (`002-e2ee-cross-signing`, Phase 3): 0% Implemented in Rust source (0 of 15 tasks in `tasks.md` completed).**
   - `Cargo.toml:11, 31-32, 67`: Still pins `rust-version = "1.88"`, `matrix-sdk = "0.10"`, `ruma = "0.12"`, and `e2ee = ["matrix-sdk/e2e-encryption"]` (no `0.19`, no `bundled-sqlite`).
   - `src/matrix/client.rs:35`: Still hardcodes `device_id: OwnedDeviceId::from("MCP_SERVER")` and does not configure a crypto store or cross-signing.
   - `src/config.rs:29-30, 111-114`: Parses `E2EE_ENABLED` and `CRYPTO_STORE_PATH`, but `validate()` (`src/config.rs:124-140`) does not check them and nothing in `src/` reads them. `MATRIX_RECOVERY_KEY_FILE` is not in `src/config.rs`.
   - `src/mcp/server.rs`: Contains zero `#[cfg(feature = "e2ee")]` blocks, zero `-r2` tools, and no `EncryptedRoomError` guard in `get_room_messages` (`server.rs:302-317`) or `send_message` (`server.rs:505-521`).
   - `src/main.rs:86-94`: `/health` returns only `status`, `server`, `version`, and `user_id` (no `e2ee` or `crossSigned` fields).
   - `Dockerfile:21`: Still uses `FROM rust:1.88-bookworm AS builder`.

### By Executed Tests

- **Observed verification:** Per audit constraints, no test commands were executed during the audit.
- **Evidence in test files:**
  - `tests/config_tests.rs:1-55` has 3 unit tests for `Config::from_env()` and `Config::validate()`.
  - `tests/cache_tests.rs:1-21` has 2 unit tests testing `ClientCache` on empty state only.
  - `tests/server_tool_list_tests.rs:1-16` has 1 compile-time trait bound check (`_assert_server_handler::<MatrixMcpServer>()`), and does not actually inspect the registered tool list.
  - `tests/integration_tests.rs:260-443` has 7 `wiremock` tests covering 6 of the 20 v1 tools (`list_joined_rooms`, `get_room_info`, `get_room_members`, `get_my_profile`, `get_notification_counts`, `send_message`). The remaining 14 v1 tools have no tests.
  - Evidence from IDE terminal log (`terminals/2.txt:200-230`): a previous `cargo run` on Oct 25 compiled `dev` profile with 6 dead-code warnings (`auth.rs`, `error.rs`, `cache.rs`).

---

## 6. Contradictions, Duplications, Unsupported Assumptions, Placeholders, and Missing Acceptance Criteria

### A. Contradictions Across Artifacts

1. **Internal contradiction on Trust Policy (`TOFU` vs. `cross-signed`) inside `spec/matrix-mcp-v1-v2.yml`:**
   - **Documented requirement A:** `spec/matrix-mcp-v1-v2.yml:19` (`non_goals`) states: `"Trust-on-first-use. Room keys are not shared with devices that are not cross-signed."` Lines 50-56 (`TrustPolicy`) state: `"enum: ['cross-signed']"`, `"'tofu' is rejected."`
   - **Documented requirement B (contradictory):** Only 6 lines below `non_goals`, `spec/matrix-mcp-v1-v2.yml:25-26` (`operational_requirements`) still states:
     - `"v2 default trust policy is TOFU (trust-on-first-use)."`
     - `"Strict verification policy is supported later and MUST NOT break TOFU default behavior."`
   - Similarly, `spec/E2EE-Prep-Checklist-for-Rust-MCP-Server.md:230-231` has `[x] TOFU (trust on first use) for agent-human DMs`.
2. **v1 Tool Response Shapes & Parameter Names: `spec/matrix-mcp-v1-v2.yml` vs. `src/mcp/server.rs` (and TS reference):**
   - **Documented requirement:** Constitution Principle I (`.specify/memory/constitution.md:20-27`) and `spec/matrix-mcp-v1-v2.yml:90-94` state that v1 tools match `mjknowles/matrix-mcp-server` behavior and schemas.
   - **Contradiction in `spec/matrix-mcp-v1-v2.yml`:**
     - `send-message` request in `spec/matrix-mcp-v1-v2.yml:184-185` specifies properties `{ roomId, body }`. In `src/mcp/server.rs:107-118` (and TS `reference/ts-source/src/schemas/toolSchemas.ts`), the parameter is `message` (plus optional `messageType`, `replyToEventId`), **not** `body`.
     - `spec/matrix-mcp-v1-v2.yml:105-190` defines structured JSON object responses for v1 tools (`{ rooms: [...] }`, `{ messages: [...] }`, `{ eventId: "..." }`), whereas `src/mcp/server.rs:259-660` (matching TS `matrix-mcp-server`) returns formatted human-readable plain text strings via `CallToolResult::success(vec![Content::text(...)])`.
3. **v2 Tool Parameter Naming (`create-room-r2` & `send-message-r2`) vs. v1 and camelCase Rules:**
   - **Documented requirement:** `.cursor/rules/mcp-tools.mdc:22-29` and `src/AGENTS.md:99` state: *"All tool input fields use camelCase JSON names via serde rename... Never use snake_case in the JSON API."*
   - **Contradiction:** `spec/matrix-mcp-v1-v2.yml:272, 293` and `.specify/specs/002-e2ee-cross-signing/spec.md:63` name the parameter `encryption_mode` (`snake_case`) alongside camelCase fields (`isDirect`, `roomId`, `forceRecreate`). Furthermore, `create-room` (v1) uses `roomName`, `isPrivate`, `inviteUsers` (`src/mcp/server.rs:130-145`), whereas `create-room-r2` in `spec/matrix-mcp-v1-v2.yml:264-273` changes these to `name`, `isDirect`, `invitees`.
4. **`bootstrap-e2ee-r2` Response Fields Across Three Specs:**
   - `.specify/specs/002-e2ee-cross-signing/spec.md:52`, `data-model.md:30-38`, and `spec/E2EE-Key-Management-Model.md:127, 192` require `crossSigned: bool` (and `data-model.md:38` adds `keyBackupRestore`) on `bootstrap-e2ee-r2`.
   - `spec/matrix-mcp-v1-v2.yml:232-253` defines `bootstrap-e2ee-r2` response properties (`deviceId`, `identityKey`, `olmEnabled`, `cryptoStoreState`, `uploadedOneTimeKeys`, `backupConfigured`, `resolvedBackupSource`, `trustPolicy`, `notes`) -- **`crossSigned` and `keyBackupRestore` are missing from the YAML contract**, while `olmEnabled` and `uploadedOneTimeKeys` are in the YAML contract but omitted from `data-model.md:30-38`.
5. **Tool Naming in `.cursor/rules/rust-e2ee.mdc` vs. Spec and Constitution:**
   - `.cursor/rules/rust-e2ee.mdc:154-157` lists planned v2 tools as `bootstrap_e2ee`, `create_encrypted_private_room`, `get_encryption_status`, and says existing `send_message` / `get_room_messages` transparently encrypt/decrypt.
   - `.specify/memory/constitution.md:142` and `.specify/specs/002-e2ee-cross-signing/spec.md:90-97` mandate `kebab-case-r2` tools (`bootstrap-e2ee-r2`, `create-room-r2`, `send-message-r2`, `get-room-messages-r2`, `get-encryption-status-r2`) and require v1 `send-message` / `get-room-messages` to **refuse** encrypted rooms.
6. **MSRV and SDK Version Rules in `.cursor/rules/project-context.mdc` & `src/AGENTS.md` vs. Constitution & `002` Spec:**
   - `.cursor/rules/project-context.mdc:14-18` says *"Key Versions (pin in all code): matrix-sdk = 0.10 ... ruma = 0.12 ... Rust edition 2021, MSRV 1.88"*.
   - `.specify/specs/002-e2ee-cross-signing/tasks.md:10` (`T001`) and `.cursor/rules/rust-e2ee.mdc:14-20` require bumping `Cargo.toml` to `matrix-sdk = 0.19` and `rust-version = 1.96`.
7. **Transport Description in `.a0proj/instructions/project-instructions.md`:**
   - `.a0proj/instructions/project-instructions.md:12, 18` mandates *"Same SSE transport"*, whereas `src/main.rs:71-80` and `src/AGENTS.md:88` use Streamable HTTP (`StreamableHttpService` at `/mcp`, explicitly not legacy SSE).

### B. Duplicated Requirements & Files

- `spec/` and `spec.bak/` contain 5 identical files each (`E2EE-Key-Management-Model.md`, `E2EE-Landscape-Review-20261023.md`, `E2EE-Landscape-Review-October-20261023.md`, `E2EE-Prep-Checklist-for-Rust-MCP-Server.md`, `matrix-mcp-v1-v2.yml`).
- `spec/E2EE-Landscape-Review-20261023.md` and `spec/E2EE-Landscape-Review-October-20261023.md` are two versions of the same review committed in the same commit (`fa70f4a`).
- `.specify/specs/001-e2ee-support/spec.md` and `.specify/specs/002-e2ee-cross-signing/spec.md` overlap on E2EE requirements, though `001` has a deprecation header pointing to `002`.

### C. Unsupported Assumptions

1. **Rust >= 1.96 toolchain availability (`spec.md:24`, `tasks.md:10`):**
   - `Dockerfile:21` is pinned to `rust:1.88-bookworm`, and `.github/workflows/docker-publish.yml:25` comments on `rust:1.88-bookworm`. Neither `Dockerfile` nor CI is included in `tasks.md` (`T001` only mentions `Cargo.toml`), so Docker builds will break as soon as `T001` bumps `rust-version = "1.96"` and `matrix-sdk = "0.19"`.
2. **Secret Manager (`backupPassphraseRef`) Resolution (`spec/E2EE-Key-Management-Model.md:39-41`, `data-model.md:22`, `tasks.md:21`):**
   - `T006` requires implementing recovery-secret resolution starting with `backupPassphraseRef` (secret manager reference), and `E2EE-Key-Management-Model.md:40` writes `secrets.get(request["backupPassphraseRef"])`.
   - **Unsupported assumption:** There is no secret manager client, SDK, CLI, or URI scheme defined anywhere in `Cargo.toml`, `src/config.rs`, or the specification (e.g., Vault, AWS SM, 1Password, Kubernetes secrets, or local file mapping). It is impossible to implement `backupPassphraseRef` deterministically from current docs.
3. **Account Password for Matrix UIAA During Cross-Signing Bootstrap (`spec.md:50-58`, `tasks.md:22`):**
   - Creating a new cross-signing identity on a Matrix homeserver (`bootstrap_cross_signing`) typically requires User-Interactive Authentication (UIAA) or account password, as noted in `spec.md:19` (*"Password-in-env is only for creating or repairing Secret Storage"*). However, `src/config.rs` only accepts `MATRIX_ACCESS_TOKEN` (no `MATRIX_PASSWORD` field exists in `src/config.rs`, despite `src/AGENTS.md:72` claiming `MATRIX_USERNAME` / `MATRIX_PASSWORD` exist).
4. **Missing Test Plan Reference (`spec/matrix-mcp-v1-v2.yml:373-374`):**
   - `spec/matrix-mcp-v1-v2.yml:374` references `file: matrix-mcp-test-plan.md`, which does not exist in this repository (it exists only in the sibling TypeScript repo `/home/l0r3zz/Dropbox/GEN-AI/matrix-mcp-server/docs/matrix-mcp-test-plan.md`).

### D. Incomplete Placeholders

- `AGENTS.md:81-83`: Unindexed DOX tree placeholder.
- `.specify/memory/constitution.md:1-13`: Contains the temporary `<!-- Sync Impact Report ... -->` HTML comment.
- `.specify/templates/plan-template.md`, `spec-template.md`, `tasks-template.md`: Uncommitted working-tree edits replace project-customized templates with raw Spec Kit v1.2.3 placeholders.

### E. Requirements Without Acceptance Criteria

1. **FR-009 (`.specify/specs/002-e2ee-cross-signing/spec.md:98`):** *"`/health` includes `e2ee` and `crossSigned` when the feature is on."* -- Has no user story or acceptance scenario defining the exact JSON types/values when E2EE is compiled in but `E2EE_ENABLED=false`, or before `bootstrap-e2ee-r2` has been called.
2. **FR-007 for the other 18 v1 tools (`.specify/specs/002-e2ee-cross-signing/spec.md:75, 96`):** User Story 4 specifies that `get-room-messages` and `send-message` return `EncryptedRoomError` on encrypted rooms, while FR-007 says *"v1 tools return `EncryptedRoomError` on encrypted rooms"*. No acceptance criteria specify how `get-messages-by-date`, `identify-active-users`, or `send-direct-message` (which also read/send messages) should behave on encrypted rooms.
3. **v2 Tool Output Format (`spec/matrix-mcp-v1-v2.yml:203-367` vs `src/mcp/server.rs`):** No acceptance criterion states whether `-r2` tools must return serialized JSON strings matching `spec/matrix-mcp-v1-v2.yml` response schemas or human-readable text like the v1 tools in `src/mcp/server.rs`.
4. **`forceRecreate: true` behavior under cross-signing (`spec/matrix-mcp-v1-v2.yml:211-216` vs `.specify/specs/002-e2ee-cross-signing/spec.md:50-58`):** `001-e2ee-support` had an acceptance scenario for `forceRecreate=true` (`001/spec.md:52`), but `002-e2ee-cross-signing/spec.md` dropped it while keeping `forceRecreate` in `spec/matrix-mcp-v1-v2.yml:211`.

---

## 7. Proposed Source-of-Truth Hierarchy (Proposal Only)

To resolve conflicts between May 2026 artifacts, October 2026 artifacts, reference docs, and code, adopt the following precedence order (highest to lowest):

1. **`.specify/memory/constitution.md`** -- Governance, non-negotiable architectural principles, and locked crate/toolchain targets.
2. **`.specify/specs/002-e2ee-cross-signing/` (`spec.md` -> `data-model.md` -> `plan.md` -> `tasks.md`)** -- Canonical feature behavior, trust policy (`cross-signed` only), SDK target (`matrix-sdk` 0.19 / Rust 1.96), and implementation sequence.
3. **`spec/E2EE-Key-Management-Model.md` & `spec/E2EE-Landscape-Review-October-20261023.md`** -- Operational key lifecycle, secret resolution order, and rationale.
4. **`spec/matrix-mcp-v1-v2.yml`** -- Tool-level request/response schema contract *(subject to reconciling the lingering TOFU lines and `crossSigned` field against #2 and #3, and v1 text-vs-JSON behavior against #5)*.
5. **Current Rust implementation (`src/`, `Cargo.toml`, `tests/`) & `reference/ts-source/`** -- Authoritative for existing v1 unencrypted tool behavior and wire compatibility.
6. **Active Cursor rules (`src/AGENTS.md`, `.cursor/rules/*.mdc`)** -- Coding conventions (subordinate to #1-#4 where stale 0.10/tool-name examples remain).
7. **Superseded / Historical Artifacts (`.specify/specs/001-e2ee-support/spec.md`, `spec/E2EE-Prep-Checklist-for-Rust-MCP-Server.md`, `spec/E2EE-Landscape-Review-20261023.md`, `reference/docs/*`, `.a0proj/*`)** -- Historical context only; never used as implementation authority.

---

## 8. Five Highest-Impact Questions for the Human Project Owner

1. **Spec Kit Directory Layout (`specs/` vs `.specify/specs/`) and `.specify/feature.json`:**
   Spec Kit v1.2.3 scripts (`common.sh`, `create-new-feature.sh`) require `.specify/feature.json` and expect features in top-level `specs/`, whereas your existing specs live in `.specify/specs/002-e2ee-cross-signing/`. Should we pin `.specify/feature.json` to `{"feature_directory": ".specify/specs/002-e2ee-cross-signing"}` (keeping `.specify/specs/`), or move the feature directories to top-level `specs/` to match Spec Kit v1.2.3 defaults?
2. **Scope of `backupPassphraseRef` (Secret Manager) in `bootstrap-e2ee-r2`:**
   `data-model.md` and `E2EE-Key-Management-Model.md` list `backupPassphraseRef` as the #1 precedence source for the recovery secret, ahead of `backupPassphraseEnvVar` and `MATRIX_RECOVERY_KEY_FILE`. What concrete secret manager or URI format must `backupPassphraseRef` resolve against in the Agent-Matrix container environment, or should `backupPassphraseRef` return a "not configured/unsupported" error in v2.0 while `backupPassphraseEnvVar` and `MATRIX_RECOVERY_KEY_FILE` carry the implementation?
3. **First-Time Cross-Signing Bootstrap Authentication (UIAA / Account Password):**
   When an agent account on Continuwuity has no existing cross-signing identity, `matrix-sdk`'s `bootstrap_cross_signing` typically requires Matrix User-Interactive Auth (account password) in addition to the access token. Should we add an optional password env-var reference (e.g., `MATRIX_PASSWORD` or `MATRIX_PASSWORD_FILE`) for initial Secret Storage / cross-signing creation, or are agent accounts pre-bootstrapped / exempted on Continuwuity?
4. **v2 (`-r2`) Tool Output Format and Input Casing (`encryption_mode` vs `encryptionMode`):**
   All 20 v1 tools in `src/mcp/server.rs` return human-readable formatted text in `CallToolResult`, whereas `spec/matrix-mcp-v1-v2.yml` specifies structured JSON objects (`deviceId`, `crossSigned`, `decryptionState`, etc.). Should the five `-r2` tools return JSON-serialized objects (and should `encryption_mode` stay `snake_case` per `matrix-mcp-v1-v2.yml` or use `camelCase` per `.cursor/rules/mcp-tools.mdc`)?
5. **v1 Encrypted-Room Refusal Scope (`EncryptedRoomError`):**
   `spec/matrix-mcp-v1-v2.yml` and `tasks.md` (`T011`) specify returning `EncryptedRoomError` on `get-room-messages` and `send-message`. Should the other v1 message-reading/sending tools (`get-messages-by-date`, `identify-active-users`, `send-direct-message`) also refuse encrypted rooms when built with `--features e2ee`?

---

## 9. Smallest Useful Next Revision

Before starting code task `T001` (bumping `Cargo.toml` to `matrix-sdk` 0.19 and Rust 1.96), the smallest high-leverage revision is a **non-code state & spec alignment commit**:

1. **Fix Spec Kit feature resolution:** Create `.specify/feature.json` pointing `"feature_directory"` to `.specify/specs/002-e2ee-cross-signing` (or move `.specify/specs/*` to `specs/*` and point `feature.json` there) so `/speckit-analyze`, `/speckit-implement`, and `.specify/scripts/bash/check-prerequisites.sh` actually resolve `002-e2ee-cross-signing`.
2. **Clean up working-tree noise:** Delete the duplicate untracked `spec.bak/` directory and remove the scratch `<!-- Sync Impact Report -->` comment from `.specify/memory/constitution.md:1-13`.
3. **Reconcile the 3 internal spec contradictions in `spec/matrix-mcp-v1-v2.yml` and `.cursor/rules/rust-e2ee.mdc`:**
   - Remove the two leftover TOFU lines in `spec/matrix-mcp-v1-v2.yml:25-26`.
   - Add `crossSigned: { type: boolean }` and `keyBackupRestore` to `bootstrap-e2ee-r2`'s response in `spec/matrix-mcp-v1-v2.yml:232-253` so it matches `.specify/specs/002-e2ee-cross-signing/data-model.md:30-38`.
   - Update `.cursor/rules/rust-e2ee.mdc:154-157` tool names (`bootstrap-e2ee-r2`, `create-room-r2`, `send-message-r2`, `get-room-messages-r2`, `get-encryption-status-r2`) to match the spec and constitution.
