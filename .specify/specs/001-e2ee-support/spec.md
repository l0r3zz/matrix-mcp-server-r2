# Feature Specification: End-to-End Encryption (E2EE) Support

**Feature Branch**: `001-e2ee-support`
**Created**: 2026-05-11
**Status**: Draft
**Input**: Phase 3 of matrix-mcp-server-r2 roadmap -- add E2EE to the existing Rust MCP server so AI agents can operate in encrypted Matrix rooms.

## Context

matrix-mcp-server-r2 is a production Rust MCP server with 20 v1 tools providing full API parity with the TypeScript matrix-mcp-server. All 20 tools currently operate on unencrypted rooms only. This spec covers adding E2EE support as v2 tools (suffixed `-r2`) while preserving v1 compatibility.

### API Contract

The existing `spec/matrix-mcp-v1-v2.yml` defines the API contract for both v1 and v2 tools. This spec-kit specification describes the broader feature intent, user scenarios, and acceptance criteria. The YAML contract remains the authoritative source for tool names, parameter shapes, and response formats.

### Cryptographic Foundation

E2EE is built on two protocols implemented by `vodozemac` (pure Rust) and orchestrated by `matrix-sdk-crypto`:

| Protocol | Purpose |
|----------|---------|
| **Olm** | 1:1 encrypted channels between devices (Triple Diffie-Hellman + Double Ratchet) |
| **Megolm** | Group encryption for rooms (one sender ratchet, shared to members via Olm) |

The key exchange handshake:
1. `keys/upload` -- publish device identity keys (Ed25519 + Curve25519) and one-time prekeys
2. `keys/query` -- fetch recipient device keys
3. `keys/claim` -- claim a one-time key to bootstrap Olm session (3DH)
4. `m.room_key` to-device -- Megolm session key delivered encrypted per-device

With the `e2ee` Cargo feature flag enabled, `matrix-sdk` handles this transparently.

---

## User Scenarios & Testing

### User Story 1 - E2EE Bootstrap (Priority: P1)

An AI agent deployed as a new container instance needs to establish its cryptographic identity before it can participate in encrypted rooms. The agent operator starts the MCP server with `E2EE_ENABLED=true` and a persistent `CRYPTO_STORE_PATH`. On first run, the server bootstraps E2EE: generates device keys, uploads them to the homeserver, and optionally bootstraps cross-signing. On subsequent runs, it restores the existing crypto state from the persistent store.

**Why this priority**: Without bootstrap, no other E2EE functionality works. This is the prerequisite for everything.

**Independent Test**: Start the server with `--features e2ee` and `E2EE_ENABLED=true` against a real Continuwuity homeserver. Call `bootstrap-e2ee-r2`. Verify via Element or another Matrix client that the MCP server's device appears in the user's device list with identity keys published.

**Acceptance Scenarios**:

1. **Given** a fresh container with no crypto store, **When** `bootstrap-e2ee-r2` is called, **Then** the server generates device keys, uploads them to the homeserver, and returns the device ID and Ed25519 fingerprint.
2. **Given** an existing crypto store from a previous run, **When** the server starts, **Then** it restores the crypto session without re-uploading keys, and `bootstrap-e2ee-r2` reports the existing device identity.
3. **Given** `E2EE_ENABLED=false` (or feature not compiled), **When** `bootstrap-e2ee-r2` is called, **Then** it returns a clear error: `"E2EE not enabled. Build with --features e2ee and set E2EE_ENABLED=true"`.
4. **Given** `forceRecreate=true`, **When** `bootstrap-e2ee-r2` is called, **Then** the crypto store is cleared, new keys are generated and uploaded, and the new device identity is returned.

---

### User Story 2 - Encrypted Room Creation (Priority: P1)

An AI agent needs to create a private encrypted room for confidential agent-internal communication (self-talk, logs) or for a private human-agent DM. The `create-room-r2` tool creates a room with the `m.room.encryption` state event set from the start, with history visibility set to `joined`.

**Why this priority**: Encrypted room creation is the foundation for all encrypted communication. Without it, there are no encrypted rooms for the other tools to operate on.

**Independent Test**: Call `create-room-r2` with `encryption_mode: "e2ee"` and verify in Element that the room shows a shield icon and the `m.room.encryption` state event is present.

**Acceptance Scenarios**:

1. **Given** E2EE is bootstrapped, **When** `create-room-r2` is called with `encryption_mode: "e2ee"`, **Then** a room is created with `m.room.encryption` state using Megolm (`m.megolm.v1.aes-sha2`), history visibility is `joined`, and the response includes `encryptionEnabled: true`.
2. **Given** E2EE is bootstrapped, **When** `create-room-r2` is called with `encryption_mode: "none"` (or omitted), **Then** an unencrypted room is created, matching the behavior of the existing `create-room` v1 tool.
3. **Given** E2EE is bootstrapped, **When** `create-room-r2` is called with `encryption_mode: "e2ee"` and `invitees` specified, **Then** the room is created encrypted and invitations are sent to the specified users.
4. **Given** E2EE is NOT bootstrapped, **When** `create-room-r2` is called with `encryption_mode: "e2ee"`, **Then** it returns an error: `"E2EE not bootstrapped. Call bootstrap-e2ee-r2 first"`.

---

### User Story 3 - Send Encrypted Messages (Priority: P1)

An AI agent needs to send messages to encrypted rooms. The `send-message-r2` tool sends a message that is automatically encrypted by the SDK when the target room has encryption enabled.

**Why this priority**: Sending messages is the core agent-to-human and agent-to-agent communication primitive.

**Independent Test**: Send a message via `send-message-r2` to an encrypted room. Verify in Element that the message is received and displayed with the encryption shield. Verify on the homeserver that the stored event content is ciphertext (not plaintext).

**Acceptance Scenarios**:

1. **Given** an encrypted room and a bootstrapped E2EE session, **When** `send-message-r2` is called, **Then** the message is encrypted via Megolm and the response includes `encrypted: true`.
2. **Given** an unencrypted room, **When** `send-message-r2` is called, **Then** the message is sent in plaintext and the response includes `encrypted: false`.
3. **Given** a room where not all member devices have been key-shared with, **When** `send-message-r2` is called, **Then** the SDK automatically shares the Megolm session key with all members' devices before encrypting, and the message is sent successfully.

---

### User Story 4 - Read Encrypted Messages (Priority: P1)

An AI agent needs to read messages from encrypted rooms. The `get-room-messages-r2` tool retrieves messages and automatically decrypts them when possible, clearly indicating encryption status and any decryption failures.

**Why this priority**: Reading messages is essential for agent comprehension of conversations.

**Independent Test**: Send a message from Element to an encrypted room. Call `get-room-messages-r2` and verify the decrypted message body is returned with `encrypted: true`.

**Acceptance Scenarios**:

1. **Given** an encrypted room with messages, **When** `get-room-messages-r2` is called, **Then** messages are decrypted and returned with `encrypted: true` per message.
2. **Given** an encrypted room with a message that cannot be decrypted (missing session key), **When** `get-room-messages-r2` is called, **Then** the message is included in the response with `body: "[Encrypted message - unable to decrypt (m.megolm.v1.aes-sha2)]"` and `encrypted: true`.
3. **Given** an unencrypted room, **When** `get-room-messages-r2` is called, **Then** messages are returned normally with `encrypted: false`.
4. **Given** a room with a mix of encrypted and unencrypted messages (e.g., encryption was enabled mid-conversation), **When** `get-room-messages-r2` is called, **Then** each message correctly reflects its own encryption status.

---

### User Story 5 - v1 Tool Guardrails on Encrypted Rooms (Priority: P2)

When E2EE is enabled, the existing v1 tools must refuse to operate on encrypted rooms with a clear, standardized error message directing the user to the `-r2` tools.

**Why this priority**: Prevents agents from silently failing or leaking unencrypted data into encrypted rooms. Lower priority because it's a safety guardrail, not new functionality.

**Independent Test**: Create an encrypted room. Call `get-room-messages` (v1) against it. Verify it returns the standardized error.

**Acceptance Scenarios**:

1. **Given** an encrypted room, **When** `get-room-messages` (v1) is called, **Then** it returns error: `"room is encrypted; use *-r2 tools instead"`.
2. **Given** an encrypted room, **When** `send-message` (v1) is called, **Then** it returns the same `EncryptedRoomError`.
3. **Given** an unencrypted room, **When** any v1 tool is called, **Then** it behaves exactly as it does today (no change).
4. **Given** E2EE is NOT enabled (feature not compiled), **When** v1 tools are called on any room, **Then** they behave exactly as today (no encrypted room detection, since the SDK doesn't have encryption awareness).

---

### User Story 6 - Encryption Status Inspection (Priority: P3)

Operators and agents need visibility into the encryption state of the server and rooms. A `get-encryption-status-r2` tool reports the server's E2EE health and per-room encryption details.

**Why this priority**: Diagnostic/operational tool, not required for core E2EE functionality.

**Independent Test**: Call `get-encryption-status-r2` and verify it returns the device ID, fingerprint, and cross-signing status.

**Acceptance Scenarios**:

1. **Given** E2EE is bootstrapped, **When** `get-encryption-status-r2` is called without a `roomId`, **Then** it returns the server's device ID, Ed25519 fingerprint, Curve25519 identity key, cross-signing status, and crypto store path.
2. **Given** E2EE is bootstrapped and a `roomId` for an encrypted room, **When** `get-encryption-status-r2` is called, **Then** it returns the room's encryption algorithm, rotation period, and member device verification status.
3. **Given** E2EE is NOT enabled, **When** `get-encryption-status-r2` is called, **Then** it returns error: `"E2EE not enabled"`.

---

### Edge Cases

- What happens when the crypto store is corrupted or missing on restart? -> The server should log a warning and re-bootstrap (upload new keys).
- What happens when an agent's device is removed from a room's member list while it holds a Megolm session key? -> The SDK handles session rotation; the agent cannot decrypt messages sent after removal.
- What happens when two MCP server instances run with the same Matrix user ID but different device IDs? -> Both receive to-device messages; each has its own crypto store. This is normal Matrix multi-device behavior.
- What happens during Continuwuity federation when Megolm keys need to be shared across homeservers? -> Handled by the Matrix protocol; the SDK manages it transparently.

---

## Requirements

### Functional Requirements

- **FR-001**: Server MUST support the `e2ee` Cargo feature flag that enables `matrix-sdk/e2e-encryption`.
- **FR-002**: When `e2ee` feature is enabled and `E2EE_ENABLED=true`, server MUST initialize the matrix-sdk client with a persistent `SqliteCryptoStore` at the path specified by `CRYPTO_STORE_PATH`.
- **FR-003**: Server MUST expose `bootstrap-e2ee-r2` tool that uploads device keys and one-time prekeys to the homeserver.
- **FR-004**: Server MUST expose `create-room-r2` tool with an `encryption_mode` parameter (`"none"` | `"e2ee"`).
- **FR-005**: Server MUST expose `send-message-r2` tool that transparently encrypts when the room has `m.room.encryption` state.
- **FR-006**: Server MUST expose `get-room-messages-r2` tool that transparently decrypts and reports per-message encryption status.
- **FR-007**: Server MUST expose `get-encryption-status-r2` diagnostic tool.
- **FR-008**: When `e2ee` feature is enabled, v1 tools (`get-room-messages`, `send-message`) MUST refuse to operate on encrypted rooms with `EncryptedRoomError`.
- **FR-009**: All v2 tools MUST be conditionally compiled behind `#[cfg(feature = "e2ee")]`.
- **FR-010**: When `e2ee` feature is NOT enabled, all `-r2` tools MUST return a clear error indicating the feature is not compiled.
- **FR-011**: The `/health` endpoint MUST include E2EE status fields when the feature is enabled (device ID, bootstrap status).

### Non-Functional Requirements

- **NFR-001**: E2EE initialization (bootstrap) MUST complete within 30 seconds on a cold start.
- **NFR-002**: Encryption/decryption overhead MUST NOT add more than 100ms p95 latency to `send-message-r2` or `get-room-messages-r2` compared to their unencrypted v1 equivalents.
- **NFR-003**: Crypto store MUST survive container restarts via volume mount without data loss.
- **NFR-004**: No encryption keys, session tokens, or decrypted message content may be logged at INFO level or above.

### Key Entities

- **CryptoStore**: Persistent SQLite database holding device keys, Olm sessions, Megolm inbound/outbound sessions, and cross-signing keys. Path configured by `CRYPTO_STORE_PATH` environment variable.
- **DeviceIdentity**: The MCP server's identity within the Matrix E2EE system -- an Ed25519 signing key and Curve25519 identity key pair, plus a device ID.
- **EncryptionMode**: Enum (`"none"` | `"e2ee"`) controlling whether v2 tools create/operate on encrypted rooms. Default: `"none"`.
- **EncryptedRoomError**: Standardized error returned by v1 tools when invoked on an encrypted room. Message: `"room is encrypted; use *-r2 tools instead"`.

---

## Success Criteria

### Measurable Outcomes

- **SC-001**: `bootstrap-e2ee-r2` successfully uploads device keys and the device appears in the user's device list on Continuwuity within 30 seconds.
- **SC-002**: Messages sent via `send-message-r2` to encrypted rooms are visible (decrypted) in Element and other E2EE-capable clients.
- **SC-003**: Messages sent from Element to encrypted rooms are successfully decrypted by `get-room-messages-r2` with zero decryption failures for messages sent after bootstrap.
- **SC-004**: All 20 existing v1 tools pass their existing test suite without modification.
- **SC-005**: `cargo build` without `--features e2ee` succeeds and the server runs correctly with only v1 tools available.
- **SC-006**: `cargo build --features e2ee` succeeds and all E2EE tools are functional.
- **SC-007**: An agent-internal encrypted room can be created, messages sent and received, across a container restart (crypto store persisted).

---

## Assumptions

- Agent-Matrix Dendrite homeservers support E2EE (they do -- Dendrite supports Olm/Megolm).
- The `matrix-sdk` v0.10 `e2e-encryption` feature handles the Olm/Megolm lifecycle transparently when given a persistent store.
- The existing background sync loop in `matrix/client.rs` will pick up to-device events (key sharing) without modification.
- Container volumes are available for crypto store persistence in the Agent-Matrix deployment.
- Cross-signing bootstrap can be performed automatically without human interaction (supported since matrix-rust-sdk PR #2750).
- The `matrixbot-ezlogin` crate (v0.3.8) serves as a reference for the bootstrap flow but will not be used as a dependency; the implementation will use `matrix-sdk` directly.

---

## References

- API contract: `spec/matrix-mcp-v1-v2.yml`
- Research: `outputs/research/research_e2ee_mcp_landscape_20260402_0206Z.md` (in Cursor-Writing-Assistant-repo)
- Matrix E2EE spec: https://spec.matrix.org/latest/client-server-api/#end-to-end-encryption
- Olm/Megolm protocol specs: https://gitlab.matrix.org/matrix-org/olm/-/tree/master/docs
- matrix-sdk-crypto tutorial: https://docs.rs/matrix-sdk-crypto/latest/matrix_sdk_crypto/tutorial/index.html
- vodozemac: https://github.com/matrix-org/vodozemac
- matrixbot-ezlogin: https://github.com/m13253/matrixbot-ezlogin
- Trail of Bits audit: https://matrix.org/blog/2022/05/16/independent-public-audit-of-vodozemac-a-native-rust-implementation-of-matrix-end-to-end-encryption
