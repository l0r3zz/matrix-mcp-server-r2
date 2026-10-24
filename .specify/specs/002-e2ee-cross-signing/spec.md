# Feature Specification: Cross-Signed E2EE

**Feature Branch**: `002-e2ee-cross-signing`
**Created**: 2026-10-23
**Status**: Accepted
**Supersedes**: `.specify/specs/001-e2ee-support/spec.md` (May 2026, `matrix-sdk` 0.10, optional cross-signing, Dendrite)
**Input**: Agent-Matrix needs this server as its primary Matrix MCP on secure networks. Trust policy is cross-signing. SDK target is `matrix-sdk` 0.19.x. Rust >= 1.96.

## Role

`matrix-mcp-server-r2` is the Matrix MCP for the Agent-Matrix fleet. v1 stays the 20 unencrypted tools. v2 (`-r2`) is the encrypted path. Public-network TOFU bots are out of scope.

## Prior Art (evaluated 2026-10-23)

| Server | What we take | What we reject |
|--------|----------------|----------------|
| [qechris/matrix-mcp](https://github.com/qechris/matrix-mcp) | Rust, `matrix-sdk`, bundled SQLite, `MATRIX_STORE_PATH`, one store per device, `unable_to_decrypt` instead of a crash, recovery-key restore (`restore_key_backup`), device id taken from the session rather than a shared constant | It will send and receive before the device is verified. We will not. Interactive SAS is optional for a human operator, not the agent bootstrap |
| [jlxq0/matrix-mcp](https://github.com/jlxq0/matrix-mcp) | Cross-signing is mandatory for peers to share Megolm keys. Device id lives in a file next to the store and rotates when the volume is wiped. Headless `bootstrap_cross_signing` creates an identity only when none exists. One client per device id | Requires Synapse + MAS. This fleet's homeserver is Continuwuity. Do not copy that prerequisite |
| [Just-Insane/matrix-mcp-server](https://github.com/Just-Insane/matrix-mcp-server) | Persistent crypto dir, `MATRIX_RECOVERY_KEY` / `MATRIX_RECOVERY_KEY_FILE`, restore key backup, write a new recovery key to a 0600 file, never rely on cwd for the store. Dendrite note: encrypted DMs can fail on device-key / to-device delivery while group rooms work | Password-in-env is only for creating or repairing Secret Storage, and it must not appear in tool results |
| [elkimek/matrix-bridge](https://github.com/elkimek/matrix-bridge) and [elkimek/matrix-e2ee-bridge](https://github.com/elkimek/matrix-e2ee-bridge) | Small reminder that a lost store cannot decrypt old sessions | Default trust is TOFU / `ignore_unverified_devices`. Do not copy |

## SDK

| Item | Requirement |
|------|-------------|
| Rust | >= 1.96 |
| `matrix-sdk` | 0.19.x, `e2e-encryption`, `bundled-sqlite`, rustls |
| `matrix-sdk-crypto` | >= 0.16.1, satisfied by 0.19 (CVE-2026-45056) |
| `rmcp` | Stay on 1.2 for this feature |
| Compiling tree today | Still `matrix-sdk` 0.10 / Rust 1.88 until task T001 |

Do not implement Olm or Megolm. Do not construct a standalone `OlmMachine` next to `Client`.

## User Scenarios

### User Story 1 - Per-agent device and store (Priority: P1)

Each agent container has its own crypto store directory and its own device id, recorded in `.device-id` inside that directory. Restart keeps both. A wiped volume creates a new id. The literal `MCP_SERVER` is not used.

**Independent Test**: Start two processes with two store paths. Their device ids differ. Restart each. The ids are unchanged.

**Acceptance Scenarios**:

1. **Given** an empty store directory, **When** the server starts with E2EE enabled, **Then** it writes `.device-id` and uploads keys for that id.
2. **Given** an existing `.device-id` and store, **When** the process restarts, **Then** the same id is restored.
3. **Given** two agents, **When** both start, **Then** they do not share a device id.

### User Story 2 - Cross-signing bootstrap (Priority: P1)

Bootstrap imports an existing cross-signing identity from Secret Storage when a recovery secret resolves, or creates one when the account has none. It refuses to reset an identity that already exists if no secret is available. The raw recovery key is never returned by an MCP tool.

**Independent Test**: On a Continuwuity account with no cross-signing, run `bootstrap-e2ee-r2`. A 0600 recovery file appears on the store volume. The tool result contains `crossSigned: true` and does not contain the key. A second call with the file removed and no secret reference fails instead of minting a new identity.

**Acceptance Scenarios**:

1. **Given** a recovery secret via secret-manager ref, env var name, or `MATRIX_RECOVERY_KEY_FILE`, **When** `bootstrap-e2ee-r2` runs, **Then** Secret Storage is unlocked and key backup is restored.
2. **Given** no cross-signing identity and no secret, **When** bootstrap runs, **Then** an identity is created and the recovery key is written to a 0600 file on the store volume.
3. **Given** an existing identity and no resolvable secret, **When** bootstrap runs, **Then** it returns an error and does not replace the identity.
4. **Given** `trustPolicy` other than `cross-signed`, **When** bootstrap runs, **Then** it returns a trust-policy error.

### User Story 3 - Encrypted group room on Continuwuity (Priority: P1)

`create-room-r2` with `encryption_mode: e2ee` creates a Megolm room. `send-message-r2` encrypts only for cross-signed member devices. `get-room-messages-r2` decrypts when the key is present and reports a structured miss when it is not.

**Independent Test**: Two cross-signed Continuwuity users in one encrypted room. A message from Element decrypts here. A message from here decrypts in Element. Restart the container. The same history still decrypts.

**Acceptance Scenarios**:

1. **Given** a bootstrapped, cross-signed device, **When** a message is sent to an encrypted room, **Then** the stored event is ciphertext and `encrypted: true`.
2. **Given** a member device that is not cross-signed, **When** a message is sent, **Then** that device is not given the room key.
3. **Given** a missing room key, **When** messages are read, **Then** the event is returned with `decryptionState: missing_key` and a non-empty body marker, not a panic.
4. **Given** an unencrypted room, **When** `encryption_mode` is `none`, **Then** behavior matches the v1 tool.

### User Story 4 - v1 refusal (Priority: P2)

v1 `get-room-messages` and `send-message` on an encrypted room return `room is encrypted; use *-r2 tools instead`. Unencrypted rooms are unchanged. A build without the `e2ee` feature still serves the 20 v1 tools.

### User Story 5 - Status (Priority: P3)

`get-encryption-status-r2` reports device id, whether the device is cross-signed, store state, and whether a recovery source is configured. It does not report secret material.

### Edge Cases

- Wiped store, same old device id: do not reuse the id. A new `.device-id` is created so `/keys/upload` does not hit a changed signing key on the old id (the failure `jlxq0/matrix-mcp` documented).
- Two processes, one store directory: refuse the second open. `jlxq0/matrix-mcp` shows that two clients on one device id both generate keys and only one is advertised.
- Encrypted DMs: group-room success is the first gate. DM is a second test. Dendrite has a known to-device failure for DMs; this fleet uses Continuwuity, and a DM failure is recorded as a homeserver result, not papered over by trusting unverified devices.

## Requirements

- **FR-001**: E2EE code compiles only with `--features e2ee` against `matrix-sdk` 0.19.x.
- **FR-002**: Store path comes from `CRYPTO_STORE_PATH` and is a volume mount, not the process cwd.
- **FR-003**: Device id is the contents of `$CRYPTO_STORE_PATH/.device-id`.
- **FR-004**: `bootstrap-e2ee-r2` cross-signs this device or restores that state from Secret Storage.
- **FR-005**: Outbound room keys go only to cross-signed devices.
- **FR-006**: `create-room-r2`, `send-message-r2`, and `get-room-messages-r2` behave as in `spec/matrix-mcp-v1-v2.yml`, with `decryptionState` on reads.
- **FR-007**: v1 tools return `EncryptedRoomError` on encrypted rooms when E2EE support is compiled in.
- **FR-008**: Recovery material never appears in logs or tool results.
- **FR-009**: `/health` includes `e2ee` and `crossSigned` when the feature is on.
- **FR-010**: `rmcp` remains 1.2 in this feature.

### Non-functional

- **NFR-001**: Cold bootstrap completes within 60 seconds, excluding a full historical key-backup download, which may run in the background and must be visible in status.
- **NFR-002**: After restart with the same volume, previously decryptable history still decrypts.

## Success Criteria

- **SC-001**: Two agents, two device ids, two stores.
- **SC-002**: Continuwuity encrypted group room round-trip with Element, then again after container restart.
- **SC-003**: A non-cross-signed device in that room cannot decrypt the message.
- **SC-004**: Existing v1 tests pass on the non-`e2ee` build.
- **SC-005**: The `e2ee` build links `matrix-sdk` 0.19.x.

## Assumptions

- Continuwuity on this fleet implements device-key upload, to-device delivery, and cross-signing queries well enough for group rooms. That is verified by SC-002, not assumed forever.
- Accounts used by agents can hold Secret Storage. Creating it on first bootstrap is allowed. Resetting an existing identity is not.
- MLS (MSC4244 / MSC4256) is not the protocol this feature implements.
