# E2EE Key Management Model for matrix-mcp-server-r2

This document is the follow-on to:

- `spec/E2EE-Prep-Checklist-for-Rust-MCP-Server.md`
- `spec/matrix-mcp-v1-v2.yml`

Its goal is to explain, in plain language, how keys should be managed for v2 E2EE.

---

## 0. Locked Decisions (Current)

These decisions are now explicit in the spec:

- **Default trust policy for v2:** `tofu`
- **Secondary trust policy:** `strict` (optional, later hardening path)
- **Backup secret input:** support both:
  - secret manager reference (`backupPassphraseRef`)
  - environment variable name (`backupPassphraseEnvVar`)
- **Backup secret precedence:**
  1. `backupPassphraseRef`
  2. `backupPassphraseEnvVar`
  3. server default fallback configuration

Raw secret values must never be passed in MCP tool arguments.

---

## 0.1 Reference Pseudocode (Bootstrap Resolution)

Use this as the source-of-truth behavior for `bootstrap-e2ee-r2`:

```python
trust_policy = request.get("trustPolicy", "tofu")

if request.get("backupPassphraseRef"):
    passphrase = secrets.get(request["backupPassphraseRef"])
    source = "secret_manager_ref"
elif request.get("backupPassphraseEnvVar"):
    passphrase = env.get(request["backupPassphraseEnvVar"])
    source = "env_var"
else:
    passphrase = config.default_backup_passphrase
    source = "server_default" if passphrase else "none"

if trust_policy == "strict" and not verification_prereqs_ready():
    raise TrustPolicyNotSatisfiedError("strict policy prerequisites not met")
```

Implementation notes:

- Never log `passphrase` (or any resolved secret value).
- Log only `source` and a boolean such as `backupConfigured`.
- If `source != "none"` and secret resolution fails, return a structured config/secret error.

---

## 1. Mental Model (Simple Version)

For each agent instance, think of E2EE as three layers:

1. **Identity layer** -- the MCP server is a Matrix device with a stable `deviceId`.
2. **Crypto state layer** -- local encrypted/structured store that keeps Olm/Megolm state.
3. **Room key layer** -- per-room/session keys used to encrypt/decrypt room messages.

If any one of these is lost, decryption behavior changes:

- Lose **identity + store**: device is effectively "new" and may not decrypt old history.
- Keep **identity + store**: restart/migration should continue decrypting old and new messages.

---

## 2. What Keys Exist

At implementation level (managed by Matrix SDK crypto), you will encounter:

- **Device identity keys** (long-lived per device)
- **One-time keys** (consumed for session setup)
- **Megolm room/session keys** (group message encryption/decryption)

The MCP server should not invent custom crypto primitives. It should rely on Matrix SDK behavior and expose state clearly through `-r2` tools.

---

## 3. Required Decisions

### 3.1 Device Identity

- One Matrix device identity per agent instance.
- Device ID is deterministic and stable per instance.
- Never reuse one device ID across multiple agents.

### 3.2 Crypto Store Persistence

- Store must be persistent and mounted on durable storage.
- Store is isolated per agent instance.
- File permissions must allow only the MCP service user.

### 3.3 Trust Mode for First Release

- Default: TOFU style trust for operational simplicity.
- Strict verification can be added later (do not block v2 rollout on full UX).

---

## 4. Lifecycle Flows

### 4.1 First Bootstrap

1. MCP starts.
2. Check for existing crypto store.
3. If missing, initialize store and device keys.
4. Upload device keys and one-time keys.
5. Return status via `bootstrap-e2ee-r2`.

Expected output includes:

- `deviceId`
- `identityKey`
- `cryptoStoreState`
- `uploadedOneTimeKeys`

### 4.2 Normal Runtime

- Sync loop processes to-device events and key updates.
- One-time keys are replenished when low.
- `send-message-r2` encrypts when `encryption_mode=e2ee`.
- `get-room-messages-r2` decrypts when keys are available.

### 4.3 Restart

- Same device ID + same crypto store => decryption continuity should remain.

### 4.4 Host Migration

- Copy crypto store with integrity checks.
- Restore before starting MCP on destination.
- Validate with `get-encryption-status-r2` and encrypted-room message read tests.

---

## 5. Error Model (Operator-Friendly)

Use explicit, structured error types for key-management states:

- `CryptoNotBootstrappedError` -- E2EE requested before bootstrap completed.
- `KeyStoreUnavailableError` -- local crypto store missing/corrupt/unreadable.
- `MissingRoomKeyError` -- encrypted event present, but room key not yet available.

These should include human-action guidance, such as:

- "Run bootstrap-e2ee-r2"
- "Restore crypto store backup"
- "Retry after sync/key share"

---

## 6. Backup and Recovery Policy

Minimum baseline:

- Backup crypto store in encrypted form only.
- Never log raw key material.
- Define retention and access controls.
- Document restore testing as part of release criteria.

Important tradeoff:

- If you intentionally recreate device/store (`forceRecreate=true`), historical decryptability may be reduced until keys are re-shared.

---

## 7. Suggested Tool Contract Additions

### 7.1 `bootstrap-e2ee-r2`

Should report:

- `deviceId`
- `identityKey`
- `cryptoStoreState`
- `uploadedOneTimeKeys`
- `backupConfigured`
- `resolvedBackupSource`
- `trustPolicy`

### 7.2 `get-encryption-status-r2`

Should report:

- `roomEncrypted`
- `bootstrapped`
- `deviceId`
- `cryptoStoreState`
- `canDecryptNow`
- `trustPolicy`
- `backupSourceConfigured`
- `notes`

### 7.3 `get-room-messages-r2`

For each message include:

- `encrypted`
- `decryptionState`: `not_encrypted | decrypted | missing_key | failed`

---

## 8. Acceptance Gates Before Declaring E2EE Ready

- Restart gate: encrypted history still decrypts after restart.
- Migration gate: encrypted history still decrypts after host migration with restored store.
- Loss gate: intentional store removal yields explicit recovery errors (not silent corruption).
- Compatibility gate: v1 behavior unchanged (`EncryptedRoomError` for encrypted rooms).

---

## 9. Practical Guidance for a Naive Operator

If you remember only three things, remember these:

1. **The crypto store is as important as the access token.** Back it up securely.
2. **A "new device" is a new trust identity.** Recreating keys is not a no-op.
3. **E2EE reliability is an operations problem, not just a code problem.** Test restart and migration paths early.
