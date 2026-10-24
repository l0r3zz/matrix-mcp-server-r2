# E2EE Key Management Model for matrix-mcp-server-r2

This document is the follow-on to:

- `spec/E2EE-Prep-Checklist-for-Rust-MCP-Server.md`
- `spec/matrix-mcp-v1-v2.yml`

Its goal is to explain, in plain language, how keys should be managed for v2 E2EE.

---

## 0. Locked Decisions (2026-10-23)

These decisions replace the earlier TOFU default. The October spec is `.specify/specs/002-e2ee-cross-signing/spec.md`.

- **Trust policy:** `cross-signed` only. There is no `tofu` mode and no `strict` opt-in.
- **Room keys:** do not send room keys or other encrypted to-device secrets to a device that is not cross-signed. Withhold with `m.room_key.withheld` / `m.unverified` via `matrix-sdk` 0.19 recipient settings.
- **Recovery secret input:** secret manager reference, environment variable name, or file path. Never a raw secret in an MCP argument, log line, or tool result.
- **Recovery secret precedence:**
  1. `backupPassphraseRef` (secret manager)
  2. `backupPassphraseEnvVar` (name only)
  3. `MATRIX_RECOVERY_KEY_FILE` (path on the store volume)
- **SDK:** `matrix-sdk` 0.19.x, Rust >= 1.96. `matrix-sdk-crypto` >= 0.16.1 (CVE-2026-45056).
- **Device id:** one id per agent, stored in the crypto-store directory. The literal `MCP_SERVER` is not allowed.

Raw secret values must never be passed in MCP tool arguments.

---

## 0.1 Reference Pseudocode (Bootstrap Resolution)

Use this as the source-of-truth behavior for `bootstrap-e2ee-r2`:

```python
trust_policy = request.get("trustPolicy", "cross-signed")
if trust_policy != "cross-signed":
    raise TrustPolicyNotSatisfiedError("only cross-signed is supported")

if request.get("backupPassphraseRef"):
    passphrase = secrets.get(request["backupPassphraseRef"])
    source = "secret_manager_ref"
elif request.get("backupPassphraseEnvVar"):
    passphrase = env.get(request["backupPassphraseEnvVar"])
    source = "env_var"
elif env.get("MATRIX_RECOVERY_KEY_FILE"):
    passphrase = read_file(env["MATRIX_RECOVERY_KEY_FILE"])
    source = "recovery_key_file"
else:
    passphrase = None
    source = "none"

if source != "none" and not passphrase:
    raise SecretResolutionError("recovery secret could not be resolved")
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

- **Device identity keys** (Ed25519 and Curve25519, long-lived per device)
- **Cross-signing keys** (master, self-signing, user-signing), held in Secret Storage
- **One-time keys** (consumed for session setup)
- **Megolm room/session keys** (group message encryption/decryption)
- **Key backup decryption key**, unlocked by the recovery key so history from before this device existed can be restored

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

### 3.3 Trust Mode

- The only mode is cross-signing. A device that is not cross-signed does not receive room keys.
- TOFU, "trust all", and `ignore_unverified_devices` are out of scope. Those are what `elkimek/matrix-bridge` and `elkimek/matrix-e2ee-bridge` do, and they do not meet this fleet's secure-network requirement.
- Headless setup follows `jlxq0/matrix-mcp` and `Just-Insane/matrix-mcp-server`: restore Secret Storage with a recovery key, or create cross-signing once when the account has none. Do not reset an existing cross-signing identity.
- Interactive emoji comparison (`qechris/matrix-mcp`) is a useful pattern for a human operator and is not required for agent bootstrap.

---

## 4. Lifecycle Flows

### 4.1 First Bootstrap

1. MCP starts and opens the persistent store.
2. Load or create `.device-id` in that directory. Never use the shared literal `MCP_SERVER`.
3. Upload device keys only after the device id and store agree.
4. If a recovery secret resolves, import cross-signing from Secret Storage and restore key backup.
5. If the account has no cross-signing identity, create one and write the new recovery key to a mode-0600 file on the store volume. Do not put that key in the tool response.
6. If an identity already exists and no recovery secret resolves, fail. Do not invent a second identity.
7. Return status via `bootstrap-e2ee-r2` (`crossSigned`, `deviceId`, `backupConfigured`, `resolvedBackupSource`). Do not return the secret.

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
- `crossSigned`
- `cryptoStoreState`
- `uploadedOneTimeKeys`
- `backupConfigured`
- `resolvedBackupSource`
- `trustPolicy` (`cross-signed` only; do not return the recovery key)

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
