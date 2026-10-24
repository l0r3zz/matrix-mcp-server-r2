# Tasks: Cross-Signed E2EE

**Input**: `.specify/specs/002-e2ee-cross-signing/`
**Prerequisites**: spec.md, plan.md, data-model.md, `spec/matrix-mcp-v1-v2.yml`

Do not start these until this plan is accepted. T001 is the first code change. Do not add `-r2` tool schemas before the 0.19 client builds.

## Phase 1: SDK pin

- [ ] T001 In `Cargo.toml`, set `rust-version` to `1.96` and `matrix-sdk` to `0.19` with `default-features = false` and features `rustls-tls` plus the `e2ee` feature mapping to `e2e-encryption` and `bundled-sqlite`. Let `ruma` follow that crate. Leave `rmcp` at 1.2.
- [ ] T002 Fix `src/matrix/client.rs` and any other call sites so `cargo build` and `cargo test` pass on the 0.19 API without enabling E2EE behavior yet. Remove the `MCP_SERVER` device id literal as part of this, even before the store file exists: device id must come from configuration or the store, not a constant.

## Phase 2: Store and device id

- [ ] T003 In `src/config.rs`, require `CRYPTO_STORE_PATH` when `E2EE_ENABLED=true`. Document `MATRIX_RECOVERY_KEY_FILE`, `backupPassphraseEnvVar`, and `backupPassphraseRef` as names and paths only.
- [ ] T004 Open the bundled SQLite store at that path. Read or create `.device-id`. Fail if another process holds the store.
- [ ] T005 Restore the access-token session with that device id and start the existing sync loop so to-device events reach the SDK.

## Phase 3: Cross-signing

- [ ] T006 Implement recovery-secret resolution in the order in `data-model.md`. Never log or return the secret.
- [ ] T007 `bootstrap-e2ee-r2`: restore Secret Storage and key backup when a secret resolves; create cross-signing and write `recovery-key` mode 0600 when the account has none; return `IdentityExistsError` when an identity exists and no secret resolves.
- [ ] T008 Configure 0.19 decryption settings and room-key recipient strategy so room keys are not shared with devices that are not cross-signed.

## Phase 4: Tools

- [ ] T009 `create-room-r2` with `encryption_mode` `none` or `e2ee`.
- [ ] T010 `send-message-r2` and `get-room-messages-r2`, including `decryptionState`.
- [ ] T011 v1 `send-message` and `get-room-messages` return `EncryptedRoomError` on encrypted rooms when the `e2ee` feature is compiled.
- [ ] T012 `get-encryption-status-r2` and `/health` fields `e2ee` and `crossSigned`.

## Phase 5: Gates

- [ ] T013 `cargo test` without `--features e2ee` passes.
- [ ] T014 `cargo test --features e2ee` passes.
- [ ] T015 Continuwuity manual gate: encrypted group room, Element round-trip, container restart, history still decrypts. Record a separate DM result. Do not weaken trust policy if the DM fails.

## Dependencies

T001 blocks everything. T002 blocks T004. T004 and T005 block T007. T008 blocks T010. T015 is last.
