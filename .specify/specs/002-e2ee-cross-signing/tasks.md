# Tasks: Unified Cross-Signed E2EE MCP Server

**Input**: `.specify/specs/002-e2ee-cross-signing/`
**Prerequisites**: `spec.md`, `plan.md`, `data-model.md`, `spec/matrix-mcp-v1-v2.yml`

T001 is the first code change.

## Phase 1: SDK 0.19 Migration

- [ ] T001 In `Cargo.toml` (and `Dockerfile`), set `rust-version` to `1.96` and `matrix-sdk` to `0.19` with `default-features = false` and features `rustls-tls` plus the `e2ee` feature mapping to `e2e-encryption` and `bundled-sqlite`. Let `ruma` follow `matrix-sdk` 0.19. Leave `rmcp` at `1.2`.
- [ ] T002 Update `src/matrix/client.rs` and `src/mcp/server.rs` call sites so `cargo build` and `cargo test` pass on the `matrix-sdk` 0.19 API without enabling E2EE behavior yet. Replace the hardcoded `"MCP_SERVER"` device id literal by resolving the session's `device_id` via `/_matrix/client/v3/account/whoami` (or configuration/store).

## Phase 2: Persistent SQLite Crypto Store & Device Identity

- [ ] T003 In `src/config.rs`, require `CRYPTO_STORE_PATH` when `E2EE_ENABLED=true`. Add `MATRIX_RECOVERY_KEY_FILE` (defaulting to `$CRYPTO_STORE_PATH/recovery-key` when `CRYPTO_STORE_PATH` is set) and optional UIAA password env/file reference for initial Secret Storage creation. Never log secret values.
- [ ] T004 When `e2ee` feature and `E2EE_ENABLED=true` are active, open the bundled SQLite store at `CRYPTO_STORE_PATH` (creating mode `0700` directory if needed). Read or initialize `$CRYPTO_STORE_PATH/.device-id` from `/_matrix/client/v3/account/whoami`, and acquire an exclusive lock so a second process cannot open the same store (`KeyStoreUnavailableError`).
- [ ] T005 Restore the `matrix-sdk` 0.19 session with the `.device-id` and SQLite crypto store, and run the background sync loop so to-device events and key updates are processed continuously.

## Phase 3: Cross-Signing Auto-Bootstrap & Recipient Trust Policy

- [ ] T006 Implement recovery-secret resolution in the precedence order defined in `data-model.md` (`backupPassphraseRef` -> `backupPassphraseEnvVar` -> `MATRIX_RECOVERY_KEY_FILE`), returning `SecretResolutionError` if `backupPassphraseRef` is passed when no external secret-manager backend is configured or if a named env var / file cannot be read. Never log or return the secret.
- [ ] T007 Implement cross-signing bootstrap (invoked automatically during startup when `E2EE_ENABLED=true` and exposed via the `bootstrap-e2ee` MCP tool): unlock Secret Storage and spawn historical key-backup restoration asynchronously in the background (tracking `keyBackupRestore: not_started | running | done | failed` so cold startup completes within 60s per `NFR-001`); create a new cross-signing identity and write `$CRYPTO_STORE_PATH/recovery-key` (mode `0600`) when the account has none on Continuwuity; return `IdentityExistsError` when an identity already exists on the homeserver and no secret resolves; and if `forceRecreate: true` is passed, recreate the local SQLite store/device keys without resetting the server-side cross-signing identity.
- [ ] T008 Configure `matrix-sdk` 0.19 decryption settings and room-key recipient strategy (`CollectStrategy`) so Megolm room keys are shared **only** with cross-signed devices (withholding keys from unverified devices via `m.unverified` and never using `TOFU` / `TrustRequirement::Untrusted`).

## Phase 4: Unified E2EE-By-Default MCP Tools & Health

- [ ] T009 Upgrade `create-room` and `send-direct-message` in `src/mcp/server.rs` with optional `encryptionMode` (`"auto"` default when E2EE is enabled, `"e2ee"`, `"none"`). Implement the pre-flight `/_matrix/client/v3/keys/query` check for `inviteUsers` / `targetUserId`: enable `m.room.encryption` (`RoomEncryptionEventContent::with_recommended_defaults()` + `HistoryVisibility::Joined`) when all invitees have cross-signed E2EE keys (or when no invitees are specified), automatically fall back to an unencrypted room with a clear notice when an invitee lacks E2EE keys in `"auto"` mode, and fail closed on network/federation errors.
- [ ] T010 Upgrade `send-message`, `get-room-messages`, `get-messages-by-date`, and `identify-active-users` in `src/mcp/server.rs` to handle encrypted and unencrypted rooms transparently on `matrix-sdk` 0.19, reporting `Encrypted: Yes/No` on sends and formatting `UnableToDecrypt` timeline events with `[Encrypted message - unable to decrypt (missing_key)]` on reads.
- [ ] T011 Add the `get-encryption-status` and `bootstrap-e2ee` MCP tools in `src/mcp/server.rs` (including `#[cfg(not(feature = "e2ee"))]` fallback stubs that return `"E2EE support not enabled; rebuild with --features e2ee"` per Constitution Principle II), and extend `GET /health` in `src/main.rs` to always include `"e2ee": <bool>` and `"crossSigned": <bool>` (defaulting to `false` when the `e2ee` feature or `E2EE_ENABLED` is off).

## Phase 5: Verification Gates

- [ ] T012 Verify `cargo test` (without `--features e2ee`) passes all unit and wiremock integration tests, including the non-`e2ee` fallback responses for `bootstrap-e2ee`, `get-encryption-status`, and `GET /health`.
- [ ] T013 Add E2EE unit/integration tests (`#[cfg(feature = "e2ee")]`) covering `.device-id` persistence, store lock contention, `/keys/query` pre-flight auto-fallback vs. E2EE room creation, and transparent encrypted send/read, and verify `cargo test --features e2ee` passes.
- [ ] T014 Continuwuity manual gate: verify encrypted group room round-trip with Element using standard `create-room`, `send-message`, and `get-room-messages`, verify history still decrypts after container restart, verify an unverified device does not receive room keys, and verify `create-room` auto-fallback with an unbootstrapped invitee.

## Dependencies

T001 blocks everything. T002 blocks T004. T004 and T005 block T007. T008 blocks T009 and T010. T014 is the final acceptance gate before starting `.specify/specs/003-integrated-matrix-bot/spec.md`.
