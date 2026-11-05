# Data Model: Unified Cross-Signed E2EE MCP Server

**Date**: 2026-10-29
**Spec**: `.specify/specs/002-e2ee-cross-signing/spec.md`

## On-Disk Layout

`$CRYPTO_STORE_PATH/` (mode `0700`, one directory per agent on persistent volume `/a0/usr/workdir/matrix-mcp-server/crypto-store`):

| Path | Contents | Lifetime |
|------|----------|----------|
| `.device-id` | Single line containing the Matrix `device_id` bound to `MATRIX_ACCESS_TOKEN` (verified via `/_matrix/client/v3/account/whoami`) | Same as the directory. Never seed with the literal `MCP_SERVER` |
| `.store.lock` / SQLite lock | Process lock preventing a second process from opening the same `CRYPTO_STORE_PATH` | Held for the lifetime of the running `matrix-mcp-server-r2` process |
| SDK SQLite files | Olm sessions, Megolm sessions, device keys, cross-signing private keys as stored by `matrix-sdk` 0.19 | Same as `.device-id` |
| `recovery-key` | Created only when this server mints a new cross-signing identity on a fresh account. Mode `0600` | Persisted on the volume (and included in `am-ctl` backups) so restarts and container rebuilds can unlock Secret Storage |

Losing the directory loses decryptability of sessions that were not in server-side key backup. Wiping the directory requires rotating the device session so `/keys/upload` never uploads new signing keys under an old `device_id`.

## Identity & Secrets

- **DeviceIdentity**: `deviceId`, Ed25519 fingerprint, Curve25519 identity key. Uploaded only after `.device-id` and the SQLite store are opened together and verified against `/whoami`.
- **CrossSigningIdentity**: Master, self-signing, and user-signing keys in Matrix Secret Storage. Either restored from the recovery secret on startup/bootstrap or created once when the account has none.
- **RecoverySecret**: Resolved in strict precedence order:
  1. `backupPassphraseRef` (secret manager reference; returns `SecretResolutionError` until an external secret-manager backend is configured)
  2. `backupPassphraseEnvVar` (name of environment variable holding the secret)
  3. `MATRIX_RECOVERY_KEY_FILE` (file path on the store volume, default `$CRYPTO_STORE_PATH/recovery-key`)
  Never logged or returned in any MCP tool response. If `forceRecreate: true` is passed to `bootstrap-e2ee`, the local SQLite store and device keys are rotated without resetting the server-side cross-signing identity (still requiring the recovery secret if an account cross-signing identity exists).
- **UiaaPassword**: Optional. Read from the file named by `MATRIX_UIAA_PASSWORD_FILE` (mode `0600` on the store volume). Used only to satisfy User-Interactive Auth on `POST /_matrix/client/v3/keys/device_signing/upload` when creating a new cross-signing identity. Never logged, never returned, never accepted as a raw MCP argument. Absent when the account already has cross-signing or when Continuwuity does not require UIAA. When the homeserver requires UIAA and the file is unset or unreadable, bootstrap returns `UiaaRequiredError` and does not create an identity.

## Tool-Visible State

- **`EncryptionMode`** (optional parameter on `create-room` and `send-direct-message`):
  - `"auto"` (default when E2EE is enabled): Pre-flight query `/_matrix/client/v3/keys/query` for all invitees. If all invitees have cross-signed E2EE keys (or no invitees are specified), enable `m.room.encryption`. If `/keys/query` succeeds and shows an invitee lacks cross-signed E2EE keys, fall back to an unencrypted room and report the fallback reason.
  - `"e2ee"`: Require E2EE (`m.room.encryption`). Return `TrustPolicyNotSatisfiedError` if any invitee lacks cross-signed E2EE keys.
  - `"none"`: Create an unencrypted room (default when E2EE is disabled).

- **`TrustPolicy`**: Fixed single value `"cross-signed"`. (`"tofu"` is rejected.)

- **`CryptoStoreState`**: `"missing"` | `"ready"` | `"corrupt"` | `"unreadable"`. When `SKIP_MATRIX_INIT=true` no store is opened; `/health` reports `e2ee: false, crossSigned: false` and the management tools return the "Matrix not initialized" error.

- **`BootstrapStatus`** (returned by `bootstrap-e2ee` and reflected in `get-encryption-status`):
  - `deviceId` (`String`)
  - `identityKey` (`String`, public Ed25519 fingerprint)
  - `crossSigned` (`bool`)
  - `cryptoStoreState` (`CryptoStoreState`)
  - `uploadedOneTimeKeys` (`usize`)
  - `backupConfigured` (`bool`)
  - `resolvedBackupSource`: `"none"` | `"secret_manager_ref"` | `"env_var"` | `"recovery_key_file"`
  - `keyBackupRestore`: `"not_started"` | `"running"` | `"done"` | `"failed"`
  - `trustPolicy`: `"cross-signed"`

- **`MessageDecryption`** (handled transparently in `get-room-messages`, `get-messages-by-date`, `identify-active-users`):
  - Plaintext events in unencrypted rooms and successfully decrypted `m.room.encrypted` events in E2EE rooms render seamlessly as `{sender}: {body}`.
  - Undecryptable events (`UnableToDecrypt`) render with an explicit marker: `{sender}: [Encrypted message - unable to decrypt ({reason})]`.

## Errors

- `TrustPolicyNotSatisfiedError` -- caller passed a policy other than `cross-signed`, or `encryptionMode: "e2ee"` was requested for an invitee without cross-signed E2EE keys, or this device is not cross-signed.
- `SecretResolutionError` -- a secret source was named and could not be resolved.
- `IdentityExistsError` -- the account already has a cross-signing identity on the homeserver and no recovery secret was available to unlock it.
- `UiaaRequiredError` -- creating a cross-signing identity requires User-Interactive Auth and `MATRIX_UIAA_PASSWORD_FILE` is unset or unreadable.
- `KeyStoreUnavailableError` -- store path missing, unreadable, or already locked by another process.
- `CryptoNotBootstrappedError` -- encrypted operation requested when E2EE bootstrap has not completed.
