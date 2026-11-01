# Feature Specification: Unified Cross-Signed E2EE MCP Server

**Feature Branch**: `002-e2ee-cross-signing`
**Created**: 2026-10-23 | **Updated**: 2026-10-29
**Status**: Accepted
**Supersedes**: `.specify/specs/001-e2ee-support/spec.md` (May 2026, `matrix-sdk` 0.10, optional cross-signing, Dendrite) and the earlier Two-Tier (`v1` refusal + `-r2` tool split) draft
**Followed By**: `.specify/specs/003-integrated-matrix-bot/spec.md` (Stage 2: absorbing `matrix-bot-rust`'s reactive `/sync` -> Agent Zero `/api/api_message` loop into `src/bot/`)
**Input**: Agent-Matrix needs `matrix-mcp-server-r2` as its unified Matrix gateway on secure networks. External MCP clients must get Cross-Signed E2EE by default on the existing 20 tool names without changing their call patterns, with automatic pre-flight `/keys/query` fallback to unencrypted rooms when an invitee lacks cross-signed E2EE keys. Trust policy inside encrypted rooms is strictly `cross-signed` (no `TOFU`). SDK target is `matrix-sdk` 0.19.x. Rust >= 1.99.

## Role

`matrix-mcp-server-r2` is the Matrix gateway for the Agent-Matrix fleet. The 20 operational MCP tools (`create-room`, `send-direct-message`, `send-message`, `get-room-messages`, `get-messages-by-date`, `identify-active-users`, etc.) transparently handle both encrypted (`m.room.encryption`) and unencrypted rooms. Two dedicated management tools (`bootstrap-e2ee` and `get-encryption-status`) provide explicit bootstrap control and readiness inspection. Public-network TOFU bots are out of scope.

## Prior Art (evaluated 2026-10-23)

| Server | What we take | What we reject |
|--------|----------------|----------------|
| [qechris/matrix-mcp](https://github.com/qechris/matrix-mcp) | Rust, `matrix-sdk`, bundled SQLite, persistent store path, one store per device, `unable_to_decrypt` marker instead of a crash, recovery-key restore (`restore_key_backup`), device id taken from the `/whoami` session rather than a shared constant | It will send and receive before the device is verified. We will not. Interactive SAS is optional for a human operator, not the agent bootstrap |
| [jlxq0/matrix-mcp](https://github.com/jlxq0/matrix-mcp) | Cross-signing is mandatory for peers to share Megolm keys. Device id lives in a file next to the store and rotates when the volume is wiped. Headless `bootstrap_cross_signing` creates an identity only when none exists. One client per device id | Requires Synapse + MAS. This fleet's homeserver is Continuwuity. Do not copy that prerequisite |
| [Just-Insane/matrix-mcp-server](https://github.com/Just-Insane/matrix-mcp-server) | Persistent crypto dir, `MATRIX_RECOVERY_KEY_FILE`, restore key backup, write a new recovery key to a 0600 file, never rely on cwd for the store. Dendrite note: encrypted DMs can fail on device-key / to-device delivery while group rooms work | Password-in-env is only for creating or repairing Secret Storage via UIAA, and it must not appear in tool results or logs |
| [elkimek/matrix-bridge](https://github.com/elkimek/matrix-bridge) and [elkimek/matrix-e2ee-bridge](https://github.com/elkimek/matrix-e2ee-bridge) | Small reminder that a lost store cannot decrypt old sessions | Default trust is TOFU / `ignore_unverified_devices`. Do not copy |

## SDK

| Item | Requirement |
|------|-------------|
| Rust | >= 1.99 (constitution v2.0.1 locked stack; `matrix-sdk` 0.19 floor is 1.96) |
| `matrix-sdk` | 0.19.x, `e2e-encryption`, `bundled-sqlite`, rustls |
| `matrix-sdk-crypto` | >= 0.16.1, satisfied by 0.19 (CVE-2026-45056) |
| `rmcp` | Stay on 1.2 for this feature |
| Compiling tree today | Still `matrix-sdk` 0.10 / Rust 1.88 until task T001 |

Do not implement Olm or Megolm. Do not construct a standalone `OlmMachine` next to `Client`.

## User Scenarios

### User Story 1 - Per-agent device and store (Priority: P1)

Each agent container has its own crypto store directory (`CRYPTO_STORE_PATH`) and its own device id, recorded in `.device-id` inside that directory and aligned with `/_matrix/client/v3/account/whoami` for the active `MATRIX_ACCESS_TOKEN`. Restart keeps both. A wiped volume creates a new id. The literal `MCP_SERVER` is not used.

**Independent Test**: Start two processes with two store paths. Their device ids differ. Restart each. The ids are unchanged.

**Acceptance Scenarios**:

1. **Given** an empty store directory, **When** the server starts with E2EE enabled, **Then** it resolves the session's device id (via `/whoami` or config), writes `.device-id`, opens the SQLite crypto store, and uploads keys for that id.
2. **Given** an existing `.device-id` and store, **When** the process restarts, **Then** the same id is restored and verified against the session.
3. **Given** two agents, **When** both start, **Then** they do not share a device id or store directory.

### User Story 2 - Automatic startup & tool-driven cross-signing bootstrap (Priority: P1)

When `E2EE_ENABLED=true`, the server automatically runs cross-signing bootstrap during startup (`create_matrix_client`), and operators can also invoke `bootstrap-e2ee` at any time. Bootstrap imports an existing cross-signing identity from Secret Storage when a recovery secret resolves, or creates one when the account has none. It refuses to reset an identity that already exists if no secret is available. The raw recovery key is never returned by an MCP tool or logged.

**Independent Test**: On a Continuwuity account with no cross-signing, start the server with `E2EE_ENABLED=true` (or run `bootstrap-e2ee`). A 0600 `recovery-key` file appears on the store volume. `/health` and `bootstrap-e2ee` report `crossSigned: true` and do not contain the key. A second run with the store/file removed and no secret reference fails with `IdentityExistsError` instead of minting a new identity.

**Acceptance Scenarios**:

1. **Given** a recovery secret via secret-manager ref, env var name, or `MATRIX_RECOVERY_KEY_FILE`, **When** startup auto-bootstrap or `bootstrap-e2ee` runs, **Then** Secret Storage is unlocked, this device is cross-signed, and key backup is restored.
2. **Given** no cross-signing identity on the homeserver and no secret, **When** bootstrap runs, **Then** an identity is created (using optional UIAA password if required by Continuwuity) and the recovery key is written to a 0600 file (`$CRYPTO_STORE_PATH/recovery-key`).
3. **Given** an existing cross-signing identity on the homeserver and no resolvable secret, **When** bootstrap runs on a fresh store, **Then** it returns `IdentityExistsError` and does not replace the identity.
4. **Given** `trustPolicy` other than `cross-signed`, **When** `bootstrap-e2ee` runs, **Then** it returns `TrustPolicyNotSatisfiedError`.

### User Story 3 - E2EE-by-default room creation with pre-flight capability fallback (Priority: P1)

When `create-room` or `send-direct-message` creates a room and E2EE is enabled, `encryptionMode` defaults to `"auto"`:
- Before creating the room, the server queries `/_matrix/client/v3/keys/query` for all invited users (`inviteUsers` / `targetUserId`).
- If all invitees possess cross-signed E2EE identity keys (or no invitees are specified), the room is created **encrypted (`m.room.encryption`) by default**.
- If `/keys/query` returns a valid HTTP 200 response showing an invitee has no E2EE device keys or no cross-signing identity, the server **automatically falls back to creating an unencrypted room** and explicitly reports the fallback in the tool output.
- If `/keys/query` fails due to a network or federation error, the server **fails closed** (returns an error rather than silently downgrading).
- Callers may also explicitly pass `encryptionMode: "e2ee"` (require encryption; fail if an invitee lacks keys) or `encryptionMode: "none"` (explicitly create an unencrypted room).

**Independent Test**: Call `create-room` inviting a cross-signed peer on Continuwuity -> room is created with `Encrypted: Yes`. Call `create-room` inviting an unbootstrapped account -> room is created with `Encrypted: No (fallback: @user:domain has no cross-signed E2EE keys)`.

**Acceptance Scenarios**:

1. **Given** `create-room` (or `send-direct-message`) with `encryptionMode` omitted (`"auto"`) and cross-signed invitees, **When** the room is created, **Then** `m.room.encryption` is enabled and `Encrypted: Yes` is reported.
2. **Given** `create-room` with `encryptionMode: "auto"` and an invitee that has no cross-signed E2EE keys in `/keys/query`, **When** the room is created, **Then** the server falls back to an unencrypted room and reports the fallback reason in the tool response.
3. **Given** `create-room` with `encryptionMode: "e2ee"` and an invitee lacking cross-signed keys, **When** called, **Then** it returns `TrustPolicyNotSatisfiedError` instead of downgrading.
4. **Given** `create-room` with `encryptionMode: "none"`, **When** called, **Then** it creates an unencrypted room directly.

### User Story 4 - Transparent messaging and reading across encrypted and unencrypted rooms (Priority: P1)

Existing tools (`send-message`, `send-direct-message`, `get-room-messages`, `get-messages-by-date`, `identify-active-users`) work seamlessly on both encrypted and unencrypted rooms without changing tool names:
- In an encrypted room, `send-message` encrypts via Megolm and shares room keys **only** with cross-signed member devices.
- In an unencrypted room (such as `Fleet HQ Clean` or a fallback room), `send-message` sends standard plaintext `m.room.message`.
- On reads (`get-room-messages`, `get-messages-by-date`, `identify-active-users`), `matrix-sdk` 0.19 automatically decrypts encrypted timeline events when the Megolm key is present and formats undecryptable events with a clear marker (`[Encrypted message - unable to decrypt (missing_key)]`) instead of panicking or dropping them.

**Independent Test**: Two cross-signed Continuwuity users in one encrypted room. A message from Element decrypts via `get-room-messages`. A message sent via `send-message` decrypts in Element. Restart the container; the same history still decrypts. Calling `send-message` and `get-room-messages` in an unencrypted room (`Fleet HQ Clean`) also succeeds unchanged.

**Acceptance Scenarios**:

1. **Given** a bootstrapped, cross-signed device, **When** `send-message` is called on an encrypted room, **Then** the stored event is ciphertext (`m.room.encrypted`) and the tool output indicates `Encrypted: Yes`.
2. **Given** a member device in an encrypted room that is not cross-signed, **When** `send-message` is called, **Then** that unverified device is not given the Megolm room key (`m.room_key.withheld` / `m.unverified`).
3. **Given** a missing room key for an event in an encrypted room, **When** `get-room-messages` is called, **Then** the event is returned with a clear `[Encrypted message - unable to decrypt (missing_key)]` marker, not a panic.
4. **Given** an unencrypted room, **When** `send-message` or `get-room-messages` is called, **Then** plaintext messages are sent and read normally without involving `TOFU`.

### User Story 5 - Encryption status and health observability (Priority: P2)

`get-encryption-status` reports device id, whether the device is cross-signed, crypto store state, whether a recovery source is configured, `keyBackupRestore` progress (`not_started | running | done | failed`), and (when `roomId` is provided) whether the room is encrypted and decryptable. `GET /health` always includes `"e2ee": <bool>` and `"crossSigned": <bool>` (both `false` when compiled without `--features e2ee`, when `E2EE_ENABLED=false`, or when `SKIP_MATRIX_INIT=true`) for deterministic `am-ctl` health checks. When compiled without `--features e2ee`, `bootstrap-e2ee` and `get-encryption-status` return `"E2EE support not enabled; rebuild with --features e2ee"`. Neither endpoint ever exposes secret material.

## Edge Cases

- **Wiped store, same old device id**: Do not reuse an old device id with newly generated identity keys. On a fresh store, align `.device-id` with `/whoami` (or a fresh login device) before uploading keys so `/keys/upload` never hits a changed signing key on an old id.
- **Two processes, one store directory**: Refuse the second open (`KeyStoreUnavailableError`). Two clients on one device id both generate keys and only one is advertised. (Stage 2 `003-integrated-matrix-bot` absorbs `matrix-bot` into this single process for this exact reason.)
- **Encrypted DMs**: Group-room success is the first gate (`SC-002`). DM is a second test (`T014`): `send-direct-message` with `encryptionMode: "auto"` to a cross-signed Continuwuity user must create an encrypted DM that round-trips with Element. Continuwuity DM behavior is recorded as a homeserver result, never papered over by trusting unverified devices.
- **`SKIP_MATRIX_INIT=true`** (existing MCP-protocol-only mode): no crypto store is opened, no `.device-id` is written, and startup auto-bootstrap does not run. `GET /health` reports `"e2ee": false, "crossSigned": false`; `bootstrap-e2ee` and `get-encryption-status` return the same "Matrix not initialized" error the other tools return in this mode.

## Requirements

- **FR-001**: E2EE code compiles with `--features e2ee` against `matrix-sdk` 0.19.x; non-`e2ee` builds continue to compile and serve unencrypted rooms (with `#[cfg(not(feature = "e2ee"))]` fallback stubs on `bootstrap-e2ee` and `get-encryption-status` returning `"E2EE support not enabled; rebuild with --features e2ee"`).
- **FR-002**: Store path comes from `CRYPTO_STORE_PATH` and is a persistent volume mount, not the process cwd.
- **FR-003**: Device id is stored in `$CRYPTO_STORE_PATH/.device-id` and aligned with the access token's server-side `device_id` via `/_matrix/client/v3/account/whoami`.
- **FR-004**: When `E2EE_ENABLED=true` (and `SKIP_MATRIX_INIT` is not set), startup automatically runs cross-signing bootstrap (and `bootstrap-e2ee` remains callable as an MCP tool, with `forceRecreate: true` rotating the local SQLite store/device keys without resetting the server-side cross-signing identity).
- **FR-005**: Inside encrypted rooms, outbound Megolm room keys go only to cross-signed devices (`TOFU` is forbidden).
- **FR-006**: `create-room` and `send-direct-message` accept optional `encryptionMode` (`"auto"` default when E2EE is enabled, `"e2ee"`, `"none"`), performing a pre-flight `/keys/query` check in `"auto"` mode to enable E2EE when invitees are cross-signed or fall back to an unencrypted room when an invitee lacks E2EE keys.
- **FR-007**: `send-message`, `get-room-messages`, `get-messages-by-date`, and `identify-active-users` transparently handle both encrypted and unencrypted rooms.
- **FR-008**: Recovery material and passwords never appear in logs or tool results; `backupPassphraseRef` returns `SecretResolutionError` until a secret-manager backend is configured.
- **FR-009**: `GET /health` always includes `"e2ee": <bool>` and `"crossSigned": <bool>` (both `false` when compiled without `--features e2ee`, when `E2EE_ENABLED=false`, or when `SKIP_MATRIX_INIT=true`).
- **FR-010**: `rmcp` remains 1.2 in this feature.

### Non-functional

- **NFR-001**: Cold bootstrap completes within 60 seconds by running historical key-backup restoration asynchronously in a non-blocking background task that updates `keyBackupRestore` (`not_started | running | done | failed`).
- **NFR-002**: After restart with the same volume, previously decryptable history still decrypts.

## Success Criteria

- **SC-001**: Two agents, two device ids, two isolated stores.
- **SC-002**: Continuwuity encrypted group room round-trip with Element using standard `create-room`, `send-message`, and `get-room-messages` tools, then again after container restart.
- **SC-003**: A non-cross-signed device in that encrypted room cannot decrypt the message (`m.room_key.withheld`).
- **SC-004**: Calling `create-room` (`encryptionMode: "auto"`) with an unbootstrapped invitee cleanly falls back to an unencrypted room and reports `Encrypted: No (fallback: ...)`, while unencrypted rooms (e.g., `Fleet HQ Clean`) continue to work unchanged.
- **SC-005**: Existing tests pass on both `--no-default-features` (non-`e2ee`) and `--features e2ee` builds on `matrix-sdk` 0.19.x.
- **SC-006**: Automated tests (`T013`) cover every error-path decision: `/keys/query` network failure fails closed, recovery-secret precedence and `SecretResolutionError`, no secret material in logs or tool output, `IdentityExistsError` on a fresh store against an account that already has cross-signing, `keyBackupRestore` state transitions without blocking bootstrap, the `[Encrypted message - unable to decrypt (...)]` marker, and `trustPolicy != "cross-signed"` rejection.
