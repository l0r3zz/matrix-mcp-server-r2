# E2EE Prep Checklist for Rust Matrix MCP Server

## 1. Spec and API Design

- [ ] Confirm versioning strategy:
  - [ ] MCP server identifier will be `matrix-mcp-server-r2`.
  - [ ] v1 tools keep original names and behavior (unencrypted only).
  - [ ] v2 tools use `-r2` suffix (e.g., `create-room-r2`, `send-message-r2`).

- [ ] Define v1 behavior on encrypted rooms:
  - [ ] Keep the same error **code** as current `matrix-mcp-server`.
  - [ ] Change error **message** to:  
        `room is encrypted; use *-r2 tools instead`
  - [ ] Ensure this is documented as `EncryptedRoomError` in the spec.

- [ ] Define v2 (`-r2`) behavior:
  - [ ] Add `encryption_mode` parameter (`"none"` | `"e2ee"`) with default `"none"`.
  - [ ] For `*-r2` tools:
    - [ ] If `encryption_mode = "none"`, behave like v1 equivalent.
    - [ ] If `encryption_mode = "e2ee"`, use E2EE where appropriate.

- [ ] Decide initial E2EE use‑cases:
  - [ ] First: agent‑internal encrypted rooms.
  - [ ] Second: human ↔ agent encrypted DMs.

---

## 2. Spec‑kit YAML Spec

- [ ] Create `matrix-mcp-server-r2.yml` (or similar) and include:

### 2.1. `spec` block

- [ ] Add high‑level intent:
  - [ ] Goals: v1 compatibility, v2 E2EE, explicit setup tools.
  - [ ] Non‑goals: no breaking v1, no auto‑E2EE, no full human verification UX.

### 2.2. `concepts` block

- [ ] Define:
  - [ ] `RoomId` (string, Matrix room ID).
  - [ ] `UserId` (string, Matrix user ID).
  - [ ] `EncryptionMode`:
    - [ ] Type: string.
    - [ ] Enum: `["none", "e2ee"]`.
    - [ ] Default: `"none"`.
  - [ ] `EncryptedRoomError`:
    - [ ] Description includes:
      - [ ] Error code same as existing server.
      - [ ] Error message: `room is encrypted; use *-r2 tools instead`.

### 2.3. `apis.v1` block (compat layer)

- [ ] Add v1 tools with behavior notes:

  - [ ] `list-joined-rooms`
    - [ ] Request/response shape matching existing server.
    - [ ] `error_behavior` documenting:
      - [ ] Same auth/sync errors as current implementation.

  - [ ] `get-room-info`
    - [ ] Request: `roomId`.
    - [ ] Response fields: `roomId`, `name`, `topic`, `isDirect`, `memberCount`, `createdAt`, `encryptionEnabled`.
    - [ ] `error_behavior` for missing/unknown rooms and Matrix API errors.

  - [ ] `get-room-messages`
    - [ ] Request: `roomId`, `limit`.
    - [ ] Response: list of messages with `eventId`, `sender`, `body`, `timestamp`.
    - [ ] If room is encrypted:
      - [ ] Do not decrypt.
      - [ ] Return `EncryptedRoomError`.

  - [ ] `send-message`
    - [ ] Request: `roomId`, `body`.
    - [ ] Response: `eventId`.
    - [ ] If room is encrypted:
      - [ ] Return `EncryptedRoomError`.

  - [ ] For the remaining v1 tools (from `matrix-mcp-test-plan.md`):
    - [ ] `search-public-rooms`
    - [ ] `get-messages-by-date`
    - [ ] `get-direct-messages`
    - [ ] `get-room-members`
    - [ ] `identify-active-users`
    - [ ] `get-all-users`
    - [ ] `get-user-profile`
    - [ ] `get-my-profile`
    - [ ] `get-notification-counts`
    - [ ] Document request/response and error semantics aligned with existing behavior.

### 2.4. `apis.v2` block (E2EE extensions)

- [ ] Add E2EE‑related tools:

  - [ ] `bootstrap-e2ee-r2`
    - [ ] Purpose: initialize and verify E2EE state (keys, store, upload).
    - [ ] Request: `forceRecreate` (bool, default false).
    - [ ] Response: `deviceId`, `identityKey`, `olmEnabled`, `notes`.
    - [ ] Behavior notes for first run vs subsequent runs.

  - [ ] `create-room-r2`
    - [ ] Request: `name`, `isDirect`, `invitees`, `encryption_mode`.
    - [ ] Response: `roomId`, `encryptionEnabled`.
    - [ ] Behavior:
      - [ ] `encryption_mode = "none"` → v1‑like create.
      - [ ] `encryption_mode = "e2ee"` → set `m.room.encryption`; requires E2EE bootstrap.

  - [ ] `send-message-r2`
    - [ ] Request: `roomId`, `body`, `encryption_mode`.
    - [ ] Response: `eventId`, `encrypted` (bool).
    - [ ] Behavior:
      - [ ] `"none"` → send plaintext.
      - [ ] `"e2ee"` → encrypt for encrypted rooms; fail clearly if crypto not bootstrapped.

  - [ ] `get-room-messages-r2`
    - [ ] Request: `roomId`, `limit`.
    - [ ] Response: messages with `eventId`, `sender`, `body`, `encrypted`, `timestamp`.
    - [ ] Behavior:
      - [ ] Unencrypted rooms: same as v1.
      - [ ] Encrypted rooms:
        - [ ] Attempt decryption.
        - [ ] On success: return plaintext with `encrypted: true`.
        - [ ] On failure: return structured decryption error.

- [ ] Plan future v2 tools (can be TODO for now):
  - [ ] `get-encryption-status-r2`.
  - [ ] Possibly key export/backup endpoints.

### 2.5. `tests` block

- [ ] Link to your existing v1 test plan:
  - [ ] `v1_compat` pointing at `matrix-mcp-test-plan.md`.
- [ ] Define v2 E2EE scenarios:
  - [ ] Agent‑internal encrypted room creation and messaging.
  - [ ] v1 tools refusing encrypted rooms with `EncryptedRoomError`.
  - [ ] `send-message-r2` / `get-room-messages-r2` working in encrypted rooms.

---

## 3. Background Reading – Matrix E2EE

- [ ] Read Matrix E2E implementation guide:
  - [ ] Understand Olm (1:1) vs Megolm (group) ratchets.
  - [ ] Understand device identity keys and one‑time keys.
  - [ ] Understand `m.room.encrypted` events and how ciphertext is wrapped.

- [ ] (Optional, deeper) Read older implementation docs or blog posts for intuition about:
  - [ ] Session establishment and rotation.
  - [ ] Security properties (forward secrecy, authenticity).

---

## 4. Background Reading – Rust E2EE (Matrix SDK)

- [ ] Read `matrix-sdk-crypto` tutorial:
  - [ ] How to instantiate and persist `OlmMachine` / crypto state.
  - [ ] How to upload keys and process sync changes.
  - [ ] How to decrypt events using the SDK.

- [ ] Skim `matrix-sdk-crypto` crate documentation:
  - [ ] Identify key types and methods you will likely use in MCP:
    - [ ] Crypto store setup.
    - [ ] Encryption/decryption calls.
    - [ ] Handling missing sessions/keys.

- [ ] Skim `matrix-rust-sdk` examples/issues:
  - [ ] Look for examples that send/receive messages in encrypted rooms.
  - [ ] Note how E2EE is integrated into a client event loop.

---

## 5. Background Reading – Applied Cryptography

In *Applied Cryptography* (any edition), skim these areas for conceptual grounding:

- [ ] **Symmetric ciphers and block modes**
  - [ ] Basic block ciphers (like AES or DES).
  - [ ] Block modes: CBC, CTR, CFB, OFB.
  - [ ] Why: Megolm is a ratcheting symmetric scheme; block modes give intuition on turning a block cipher into a stream and handling IVs/nonces.

- [ ] **Message authentication / MACs**
  - [ ] MACs (e.g., HMAC).
  - [ ] The idea of authenticated encryption (even if AEAD is not fully formalized).
  - [ ] Why: Matrix uses authenticated encryption; this explains how you detect tampering.

- [ ] **Public‑key crypto and key exchange**
  - [ ] Public/private key pairs.
  - [ ] Diffie–Hellman key exchange.
  - [ ] Why: Olm relies on DH (on Curve25519) to create shared secrets between devices, which then derive symmetric keys.

- [ ] **Forward secrecy and key erasure (if covered)**
  - [ ] Concepts of deriving new keys and discarding old ones.
  - [ ] Why: Ratchets (Olm/Megolm) provide forward secrecy by evolving keys over time and forgetting history.

- [ ] **Skip or lightly skim**
  - [ ] Deep protocol‑design chapters (you’re using Matrix’s protocol, not inventing one).
  - [ ] Heavy math and niche primitives not used in Matrix.

---

## 6. Tie‑back to Implementation

- [ ] After reading Matrix E2EE + `matrix-sdk-crypto`, update your spec to:
  - [ ] Add a `CryptoNotBootstrappedError` (or similar) concept for v2 tools.
  - [ ] Specify what happens when:
    - [ ] Keys are missing for an encrypted room.
    - [ ] Decryption fails due to unknown device or missing session.

- [ ] Create a small internal checklist for the MCP crypto subsystem:
  - [ ] On startup: ensure crypto store exists and bootstrap if needed.
  - [ ] On sync: feed events into the crypto engine for key updates.
  - [ ] On send in `*-r2`: call the SDK’s encryption functions when needed.
  - [ ] On receive in `*-r2`: attempt decryption, handle failures per spec.


