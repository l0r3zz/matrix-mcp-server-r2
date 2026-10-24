# Data Model: Cross-Signed E2EE

**Date**: 2026-10-23
**Spec**: `.specify/specs/002-e2ee-cross-signing/spec.md`

## On-disk layout

`$CRYPTO_STORE_PATH/` (mode 0700, one volume per agent):

| Path | Contents | Lifetime |
|------|----------|----------|
| `.device-id` | Single line, the Matrix device id | Same as the directory. Missing file means generate a new id. Never seed it with `MCP_SERVER` |
| SDK SQLite files | Olm sessions, Megolm sessions, device keys, cross-signing private keys as stored by `matrix-sdk` 0.19 | Same as `.device-id` |
| `recovery-key` | Created only when this server mints a new cross-signing identity. Mode 0600 | Operator copies it into the secret manager, then may delete the file |

Losing the directory loses decryptability of sessions that were not in server-side key backup. Gaining a new directory must generate a new device id.

## Identity

- **DeviceIdentity**: `deviceId`, Ed25519 fingerprint, Curve25519 identity key. Uploaded only after `.device-id` and the store are opened together.
- **CrossSigningIdentity**: master, self-signing, and user-signing keys in Secret Storage. Either restored from the recovery secret or created once.
- **RecoverySecret**: resolved from `backupPassphraseRef`, then `backupPassphraseEnvVar`, then `MATRIX_RECOVERY_KEY_FILE`. Not a field on the tool response.

## Tool-visible state

`TrustPolicy` is the single value `cross-signed`.

`CryptoStoreState`: `missing`, `ready`, `corrupt`, `unreadable`.

`BootstrapStatus`:

- `deviceId`
- `identityKey` (public fingerprint)
- `crossSigned` (bool)
- `cryptoStoreState`
- `backupConfigured` (bool)
- `resolvedBackupSource`: `none`, `secret_manager_ref`, `env_var`, `recovery_key_file`
- `keyBackupRestore`: `not_started`, `running`, `done`, `failed`

`MessageDecryption`:

- `encrypted` (bool)
- `decryptionState`: `not_encrypted`, `decrypted`, `missing_key`, `failed`

## Errors

- `TrustPolicyNotSatisfiedError` -- caller passed a policy other than `cross-signed`, or this device is not cross-signed and a send would have to trust an unverified peer.
- `SecretResolutionError` -- a secret source was named and did not resolve.
- `IdentityExistsError` -- the account already has cross-signing and no recovery secret was provided.
- `KeyStoreUnavailableError` -- store path missing, unreadable, or already opened by another process.
- `CryptoNotBootstrappedError` -- `-r2` encrypt path used before bootstrap.
- `EncryptedRoomError` -- v1 tool on an encrypted room. Message remains `room is encrypted; use *-r2 tools instead`.
